use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::{
    backend::renderer::{
        element::{Element, Id, Kind, RenderElement, UnderlyingStorage},
        gles::{
            element::PixelShaderElement, ffi, GlesError, GlesFrame, GlesPixelProgram, GlesRenderer,
            GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType,
        },
        utils::{CommitCounter, OpaqueRegions},
    },
    desktop::{layer_map_for_output, Space, Window},
    output::Output,
    utils::{Buffer, Logical, Physical, Rectangle, Scale, Size, Transform},
    wayland::seat::WaylandFocus,
};
use std::{error::Error, fmt, ptr};

pub(crate) const GLASS_FALLBACK_LIMITATION: &str = "Prime glass effects are in fallback mode";
const BLUR_RADIUS: i32 = 20;

const WINDOW_SHADOW_SHADER: &str = r#"
precision mediump float;
uniform vec2 size;
uniform float alpha;
uniform float strength;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

void main() {
    vec2 p = v_coords * size;
    vec2 half_size = size * 0.5;
    vec2 q = abs(p - half_size) - (half_size - vec2(30.0));
    float dist = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - 18.0;
    float fade = 1.0 - smoothstep(-4.0, 30.0, dist);
    vec3 shadow_color = mix(vec3(0.015, 0.025, 0.075), vec3(0.10, 0.045, 0.22), strength * 0.35);
    vec4 color = vec4(shadow_color, fade * strength * alpha);
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
"#;

const GLASS_SHADER: &str = r#"#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision mediump float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
uniform vec2 texel_size;
uniform vec4 material_tint;
varying vec2 v_coords;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

void main() {
    vec2 px = texel_size;
    vec4 color = texture2D(tex, v_coords) * 0.18;
    color += texture2D(tex, v_coords + vec2(px.x * 2.0, 0.0)) * 0.10;
    color += texture2D(tex, v_coords - vec2(px.x * 2.0, 0.0)) * 0.10;
    color += texture2D(tex, v_coords + vec2(0.0, px.y * 2.0)) * 0.10;
    color += texture2D(tex, v_coords - vec2(0.0, px.y * 2.0)) * 0.10;
    color += texture2D(tex, v_coords + vec2(px.x * 4.0, px.y * 2.0)) * 0.07;
    color += texture2D(tex, v_coords + vec2(px.x * 4.0, -px.y * 2.0)) * 0.07;
    color += texture2D(tex, v_coords + vec2(-px.x * 4.0, px.y * 2.0)) * 0.07;
    color += texture2D(tex, v_coords - vec2(px.x * 4.0, px.y * 2.0)) * 0.07;
    color += texture2D(tex, v_coords + vec2(px.x * 7.0, 0.0)) * 0.035;
    color += texture2D(tex, v_coords - vec2(px.x * 7.0, 0.0)) * 0.035;
    color += texture2D(tex, v_coords + vec2(0.0, px.y * 7.0)) * 0.035;
    color += texture2D(tex, v_coords - vec2(0.0, px.y * 7.0)) * 0.035;

#if defined(NO_ALPHA)
    color.a = 1.0;
#endif
    float luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    color.rgb = mix(color.rgb, vec3(luma), 0.10);
    color.rgb = mix(color.rgb, material_tint.rgb, material_tint.a);
    color.a *= alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    gl_FragColor = color;
}
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaterialKind {
    Rail,
    PrimeLauncher,
    QuickControls,
}

pub(crate) fn material_for_namespace(namespace: &str) -> Option<MaterialKind> {
    match namespace {
        "prime.shell.rail" | "prime.shell.status" => Some(MaterialKind::Rail),
        "prime.shell.prime" => Some(MaterialKind::PrimeLauncher),
        "prime.shell.quick-controls" => Some(MaterialKind::QuickControls),
        _ => None,
    }
}

pub(crate) fn expanded_capture(
    area: Rectangle<i32, Physical>,
    radius: i32,
    output_size: Size<i32, Physical>,
) -> Rectangle<i32, Physical> {
    let x1 = area.loc.x.saturating_sub(radius).max(0);
    let y1 = area.loc.y.saturating_sub(radius).max(0);
    let x2 = area
        .loc
        .x
        .saturating_add(area.size.w)
        .saturating_add(radius)
        .min(output_size.w);
    let y2 = area
        .loc
        .y
        .saturating_add(area.size.h)
        .saturating_add(radius)
        .min(output_size.h);
    Rectangle::new((x1, y1).into(), ((x2 - x1).max(0), (y2 - y1).max(0)).into())
}

pub(crate) fn shadow_area(geometry: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
    const PAD: i32 = 34;
    Rectangle::new(
        (geometry.loc.x - PAD, geometry.loc.y - PAD).into(),
        (geometry.size.w + PAD * 2, geometry.size.h + PAD * 2).into(),
    )
}

pub(crate) const fn shadow_strength(active: bool) -> f32 {
    if active {
        0.48
    } else {
        0.28
    }
}

#[derive(Debug)]
pub(crate) struct EffectsError(String);

impl fmt::Display for EffectsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for EffectsError {}

pub(crate) struct EffectsState {
    texture: GlesTexture,
    program: GlesTexProgram,
    shadow_program: GlesPixelProgram,
    output_size: Size<i32, Physical>,
}

impl EffectsState {
    pub(crate) fn new(
        renderer: &mut GlesRenderer,
        output_size: Size<i32, Physical>,
    ) -> Result<Self, EffectsError> {
        if output_size.w <= 0 || output_size.h <= 0 {
            return Err(EffectsError("Prime glass output size is empty".to_owned()));
        }
        let program = renderer
            .compile_custom_texture_shader(
                GLASS_SHADER,
                &[
                    UniformName::new("texel_size", UniformType::_2f),
                    UniformName::new("material_tint", UniformType::_4f),
                ],
            )
            .map_err(|error| EffectsError(format!("Prime glass shader compile failed: {error}")))?;

        let shadow_program = renderer
            .compile_custom_pixel_shader(
                WINDOW_SHADOW_SHADER,
                &[UniformName::new("strength", UniformType::_1f)],
            )
            .map_err(|error| {
                EffectsError(format!(
                    "Prime window shadow shader compile failed: {error}"
                ))
            })?;

        let texture_id = renderer
            .with_context(|gl| unsafe {
                let mut previous: ffi::types::GLint = 0;
                let mut texture: ffi::types::GLuint = 0;
                gl.GetIntegerv(ffi::TEXTURE_BINDING_2D, &mut previous);
                gl.GenTextures(1, &mut texture);
                gl.BindTexture(ffi::TEXTURE_2D, texture);
                gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MIN_FILTER, ffi::LINEAR as i32);
                gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MAG_FILTER, ffi::LINEAR as i32);
                gl.TexParameteri(
                    ffi::TEXTURE_2D,
                    ffi::TEXTURE_WRAP_S,
                    ffi::CLAMP_TO_EDGE as i32,
                );
                gl.TexParameteri(
                    ffi::TEXTURE_2D,
                    ffi::TEXTURE_WRAP_T,
                    ffi::CLAMP_TO_EDGE as i32,
                );
                gl.TexImage2D(
                    ffi::TEXTURE_2D,
                    0,
                    ffi::RGBA as i32,
                    output_size.w,
                    output_size.h,
                    0,
                    ffi::RGBA,
                    ffi::UNSIGNED_BYTE,
                    ptr::null(),
                );
                let error = gl.GetError();
                gl.BindTexture(ffi::TEXTURE_2D, previous as u32);
                (texture, error)
            })
            .map_err(|error| {
                EffectsError(format!("Prime glass texture allocation failed: {error}"))
            })?;
        if texture_id.0 == 0 || texture_id.1 != ffi::NO_ERROR {
            return Err(EffectsError(format!(
                "Prime glass texture allocation returned GL error 0x{:04x}",
                texture_id.1
            )));
        }
        let texture = unsafe {
            GlesTexture::from_raw(
                renderer,
                Some(ffi::RGBA),
                false,
                texture_id.0,
                Size::<i32, Buffer>::from((output_size.w, output_size.h)),
            )
        };
        Ok(Self {
            texture,
            program,
            shadow_program,
            output_size,
        })
    }

    pub(crate) fn elements_for_output(&self, output: &Output) -> Vec<(Id, GlassBackdropElement)> {
        let map = layer_map_for_output(output);
        let scale: Scale<f64> = output.current_scale().fractional_scale().into();
        map.layers()
            .filter_map(|layer| {
                let material = material_for_namespace(layer.namespace())?;
                let geometry = map.layer_geometry(layer)?;
                let surface_id = Id::from_wayland_resource(layer.wl_surface());
                Some((
                    surface_id,
                    GlassBackdropElement::new(
                        geometry,
                        material,
                        self.texture.clone(),
                        self.program.clone(),
                        self.output_size,
                        scale,
                    ),
                ))
            })
            .collect()
    }

    pub(crate) fn shadow_elements_for_output(
        &self,
        output: &Output,
        space: &Space<Window>,
        focused: Option<&WlSurface>,
    ) -> Vec<(Id, PixelShaderElement)> {
        let output_geometry = match space.output_geometry(output) {
            Some(geometry) => geometry,
            None => return Vec::new(),
        };
        space
            .elements()
            .filter_map(|window| {
                let surface = window.wl_surface()?;
                let geometry = space.element_geometry(window)?;
                if !output_geometry.overlaps(geometry) {
                    return None;
                }
                let active = focused.is_some_and(|focus| focus == surface.as_ref());
                let area = shadow_area(geometry);
                let element = PixelShaderElement::new(
                    self.shadow_program.clone(),
                    area,
                    None,
                    1.0,
                    vec![Uniform::new("strength", shadow_strength(active))],
                    Kind::Unspecified,
                );
                Some((Id::from_wayland_resource(surface.as_ref()), element))
            })
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct GlassBackdropElement {
    id: Id,
    commit: CommitCounter,
    area: Rectangle<i32, Logical>,
    material: MaterialKind,
    texture: GlesTexture,
    program: GlesTexProgram,
    output_size: Size<i32, Physical>,
    output_scale: Scale<f64>,
}

impl GlassBackdropElement {
    fn new(
        area: Rectangle<i32, Logical>,
        material: MaterialKind,
        texture: GlesTexture,
        program: GlesTexProgram,
        output_size: Size<i32, Physical>,
        output_scale: Scale<f64>,
    ) -> Self {
        Self {
            id: Id::new(),
            commit: CommitCounter::default(),
            area,
            material,
            texture,
            program,
            output_size,
            output_scale,
        }
    }

    fn tint(&self) -> (f32, f32, f32, f32) {
        match self.material {
            MaterialKind::Rail => (0.11, 0.08, 0.24, 0.18),
            MaterialKind::PrimeLauncher => (0.12, 0.08, 0.28, 0.22),
            MaterialKind::QuickControls => (0.04, 0.16, 0.22, 0.20),
        }
    }
}

impl Element for GlassBackdropElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        let physical: Size<i32, Physical> =
            self.area.size.to_physical_precise_round(self.output_scale);
        Rectangle::from_size(Size::<f64, Buffer>::from((
            physical.w as f64,
            physical.h as f64,
        )))
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.area.to_physical_precise_round(scale)
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }

    fn alpha(&self) -> f32 {
        1.0
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GlassBackdropElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        _src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        _opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let capture = expanded_capture(dst, BLUR_RADIUS, self.output_size);
        let output_height = self.output_size.h;
        let texture_id = self.texture.tex_id();
        let gl_error = frame.with_context(|gl| unsafe {
            let mut previous: ffi::types::GLint = 0;
            gl.GetIntegerv(ffi::TEXTURE_BINDING_2D, &mut previous);
            gl.BindTexture(ffi::TEXTURE_2D, texture_id);
            let gl_y = output_height - capture.loc.y - capture.size.h;
            gl.CopyTexSubImage2D(
                ffi::TEXTURE_2D,
                0,
                capture.loc.x,
                gl_y,
                capture.loc.x,
                gl_y,
                capture.size.w,
                capture.size.h,
            );
            let error = gl.GetError();
            gl.BindTexture(ffi::TEXTURE_2D, previous as u32);
            error
        })?;
        if gl_error != ffi::NO_ERROR {
            eprintln!("prime-compositor glass framebuffer capture GL error: 0x{gl_error:04x}");
            return Ok(());
        }

        let src = Rectangle::new(
            (dst.loc.x as f64, dst.loc.y as f64).into(),
            (dst.size.w as f64, dst.size.h as f64).into(),
        );
        let uniforms = [
            Uniform::new(
                "texel_size",
                (
                    1.0 / self.output_size.w as f32,
                    1.0 / self.output_size.h as f32,
                ),
            ),
            Uniform::new("material_tint", self.tint()),
        ];
        frame.render_texture_from_to(
            &self.texture,
            src,
            dst,
            damage,
            &[],
            Transform::Normal,
            0.98,
            Some(&self.program),
            &uniforms,
        )
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_prime_transient_namespaces_request_glass() {
        assert_eq!(
            material_for_namespace("prime.shell.rail"),
            Some(MaterialKind::Rail)
        );
        assert_eq!(
            material_for_namespace("prime.shell.status"),
            Some(MaterialKind::Rail)
        );
        assert_eq!(
            material_for_namespace("prime.shell.prime"),
            Some(MaterialKind::PrimeLauncher)
        );
        assert_eq!(
            material_for_namespace("prime.shell.quick-controls"),
            Some(MaterialKind::QuickControls)
        );
        assert_eq!(material_for_namespace("prime.shell.orb"), None);
        assert_eq!(material_for_namespace("random.client"), None);
        assert_eq!(material_for_namespace("prime.shell.background"), None);
    }

    #[test]
    fn blur_capture_is_clamped_to_output() {
        let area = Rectangle::new((-10, 20).into(), (200, 100).into());
        let capture = expanded_capture(area, 24, Size::from((1920, 1080)));
        assert_eq!(capture.loc.x, 0);
        assert_eq!(capture.loc.y, 0);
        assert!(capture.size.w <= 224);
        assert!(capture.size.h <= 148);
    }

    #[test]
    fn window_shadow_expands_geometry_and_active_is_stronger() {
        let geometry: Rectangle<i32, Logical> = Rectangle::new((100, 80).into(), (640, 480).into());
        let shadow = shadow_area(geometry);
        assert!(shadow.loc.x < geometry.loc.x);
        assert!(shadow.loc.y < geometry.loc.y);
        assert!(shadow.size.w > geometry.size.w);
        assert!(shadow.size.h > geometry.size.h);
        assert!(shadow_strength(true) > shadow_strength(false));
    }
}
