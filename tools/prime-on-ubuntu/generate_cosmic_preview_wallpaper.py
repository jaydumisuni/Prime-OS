"""Make an owner-reference cosmic wallpaper for isolated Prime UI review.

Source is the exact owner-provided upper mockup viewport, transferred with SHA-256
3c2bfe93eac6845f8b7de3f0e96a968acfe708adda0139da73b8f2bde2a122e6.
Never use the composite screenshot itself as fake desktop UI. The Shell draws
all interactive controls separately. Output is an approximate reconstructed
wallpaper, NOT a pixel-identical reproduction of unseen source scenery.
"""
from pathlib import Path
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ASSETS = Path(__file__).resolve().parents[2] / "crates/prime-shell/assets"
src = Image.open(ASSETS / "reference-icons/cosmic_scene_reference.webp").convert("RGB")
w, h = src.size
assert (w,h)==(1672,582)
data=np.asarray(src).astype(np.float32)
yy,xx=np.mgrid[:h,:w]
base=np.empty_like(data)
stops=[(0,(2,7,25)),(180,(2,10,30)),(320,(4,15,36)),(430,(7,18,44)),(520,(23,23,61)),(581,(30,28,68))]
for channel in range(3):
    base[:,:,channel]=np.interp(yy[:,0],[x[0] for x in stops],[x[1][channel] for x in stops])[:,None]
for cx,cy,sx,sy,color in [
    (680,395,300,245,(6,32,112)), (800,425,230,210,(2,22,102)),
    (470,555,290,100,(81,10,90)), (1220,570,380,200,(32,8,90))
]:
    amt=np.exp(-(((xx-cx)/sx)**2+((yy-cy)/sy)**2)/2)
    for channel,value in enumerate(color):base[:,:,channel]+=value*amt*.45
rng=np.random.default_rng(43501)
for _ in range(530):
    x=int(rng.integers(400,1280));y=int(rng.integers(0,480))
    if rng.random()<.3:continue
    value=int(rng.integers(30,95));radius=0 if rng.random()<.84 else 1
    base[max(0,y-radius):min(h,y+radius+1), max(0,x-radius):min(w,x+radius+1)]+=np.array((value*.35,value*.57,value))
mask=Image.new("L",(w,h),0)
d=ImageDraw.Draw(mask)
# Mask only the flattened controls, not the bright orbital ring and cosmic
# background. All coordinates refer to the provided 1672x582 top viewport.
# Owner visual gate: source control positions are masked and redrawn by
# native Shell. Preserve all ring and scenic pixels outside these regions.
unwanted=[
    (0,0,94,581),         # left navigation
    (1470,0,1671,215),   # Machine / Projects / Bin
    (781,17,898,145),    # Browser
    (636,64,751,186),    # Machine
    (920,62,1044,190),   # Settings
    (544,156,658,278),   # Terminal
    (507,271,612,391),   # Files
    (572,376,680,501),   # Music
    (687,440,795,575),   # Tools
    (887,440,993,575),   # Chat
    (1006,380,1116,510), # Gallery
    (1064,275,1178,389), # Notes
    (1030,160,1134,279), # Store
    (367,305,560,548),   # left interactive stack and arrow
    (1099,310,1296,551), # right interactive stack and arrow
    (172,457,415,575),   # left instruction callout
    (1277,462,1513,578), # right instruction callout
    (663,305,1012,361),  # flattened search field
]
for bounds in unwanted:
    d.rounded_rectangle(bounds,radius=10,fill=255)
# Central image is replaced by an untouched clean Menu-closed source emblem,
# but preserve the surrounding luminous circular ring and cosmic stars.
d.ellipse((720,161,960,421),fill=255)
mask=mask.filter(ImageFilter.MaxFilter(87)).filter(ImageFilter.GaussianBlur(19))
# Restore the original left floating monolith and light beam, not the
# nearby left-side stack. An oversized global dilation erased this real scene.
hole=Image.new("L",(w,h),0)
hd=ImageDraw.Draw(hole)
hd.polygon([(238,180),(373,180),(377,306),(365,436),(233,442)],fill=255)
hole=hole.filter(ImageFilter.GaussianBlur(11))
mask_arr=np.asarray(mask,dtype=np.float32)
hole_arr=np.asarray(hole,dtype=np.float32)/255.0
mask=Image.fromarray(np.uint8(np.clip(mask_arr*(1-hole_arr),0,255)),'L')
strength=np.asarray(mask).astype(np.float32)/255.0
# The flattened mockup embeds text *inside* its central logo preservation
# ellipse. Remove those remnants without erasing the luminous A/ring.
# Apply full alpha to actual unwanted glyph/search interiors. The broadened
# outline remains feathered by the original mask above.
rgb=np.clip(data*(1-strength[:,:,None])+base*strength[:,:,None],0,255).astype(np.uint8)
image=Image.fromarray(rgb).convert("RGBA")
# Do not redraw or replace the central artwork embedded in the approved
# reference wallpaper. The top-left Prime launcher uses the canonical mark.
# Preserve the owner's 1672 px wide geometry instead of stretching the
# 582 px desktop viewport vertically to the KRATOS 718 px output.
scale=1366 / 1672
upper_height=round(582*scale)
upper=image.convert("RGB").resize((1366,upper_height),Image.Resampling.LANCZOS)
emblem_path=ASSETS/"reference-icons/clean_menu_closed_emblem_highres.webp"
emblem=Image.open(emblem_path).convert("RGBA")
assert emblem.size==(390,355),f"Unexpected emblem asset: {emblem.size}"
emblem=emblem.resize((333,303),Image.Resampling.LANCZOS)
# A source panel is not a square wallpaper tile. Fade its peripheral
# background away, keeping the bright central mark and rings, and sharpen
# the inherited mini-panel artwork without sharpening the transparency.
from PIL import ImageEnhance
emblem_rgb=emblem.convert("RGB").filter(
    ImageFilter.UnsharpMask(radius=1.1, percent=165, threshold=2)
)
emblem_alpha=np.asarray(emblem.getchannel("A"),dtype=np.float32)
eh,ew=emblem_alpha.shape
ey,ex=np.mgrid[0:eh,0:ew]
radial=np.sqrt(((ex-ew/2)/(ew/2))**2+((ey-eh/2)/(eh/2))**2)
feather=np.clip((1.02-radial)/.24,0,1)
feather=feather*feather*(3-2*feather)
adjusted=np.clip(emblem_alpha*feather,0,255).astype(np.uint8)
emblem_rgb.putalpha(Image.fromarray(adjusted,"L"))
emblem=emblem_rgb
# The owner's wallpaper mark remains embedded in the *wallpaper* layer.
# Prime's real launcher/search is a separate Shell overlay.
layer=upper.convert("RGBA")
layer.alpha_composite(emblem,(516,96))
upper=layer.convert("RGB")
# Retain the actual owner-scene mountain silhouettes in the unoccluded
# bottom-center strip. The former full-width procedural replacement erased
# those peaks, making the sky/lake junction unnaturally flat.
reference_scaled=src.resize((1366,upper_height),Image.Resampling.LANCZOS)
mountain_start=round(572*scale)
mountain_end=upper_height
import numpy as _np
alpha_mask=Image.new("L",(1366,upper_height),0)
from PIL import ImageDraw as _ImageDraw
_a=_ImageDraw.Draw(alpha_mask)
_a.rectangle((round(470*scale),mountain_start,round(1215*scale),mountain_end),fill=255)
alpha_mask=alpha_mask.filter(ImageFilter.GaussianBlur(11))
# A narrow alpha window avoids reintroducing flattened launcher controls.
upper=Image.composite(reference_scaled,upper,alpha_mask)
lower_height=718-upper_height
ground=Image.open(ASSETS/"reference-icons/ground_extension.webp").convert("RGB")
if ground.size != (1366,lower_height):
    raise ValueError(f"Expected 1366x{lower_height} extension, got {ground.size}")
# Extend the original scene's lower terrain into the empty compositor area.
# A broad alpha blend (rather than a hard Y seam) makes both landscapes join.
start_y=upper_height-70
scaled_ground=ground.resize((1366,718-start_y),Image.Resampling.LANCZOS)
out=Image.new("RGB",(1366,718))
out.paste(upper,(0,0))
upper_np=np.asarray(upper,dtype=np.float32)
ground_np=np.asarray(scaled_ground,dtype=np.float32)
join=100
for row in range(718-start_y):
    y=start_y+row
    amount=np.clip(row/join,0,1)
    # Smoothstep avoids a visible derivative jump at either end of the crossfade.
    amount=amount*amount*(3-2*amount)
    if y<upper_height:
        blended=upper_np[y]*(1-amount)+ground_np[row]*amount
    else:
        blended=ground_np[row]
    row_image=Image.fromarray(blended.clip(0,255).astype(np.uint8)[None,:,:],"RGB")
    assert row_image.size == (1366,1)
    out.paste(row_image,(0,y))
path=ASSETS/"reference-icons/cosmic_clean_1366.png"
# A full-width lower scene is mandatory; catch accidental row-wise crop bugs
# before any build can embed an opaque black half-screen.
lower=np.asarray(out.convert("RGB"))[520:700,175:1180]
assert float(lower.mean()) > 12.0, "Cosmic lower-scene reconstruction unexpectedly blank"
# Reintroduce the owner mockup's outer orbital halo. The central emblem
# remains genuine extracted art; these are decorative glow strokes behind
# the live application sprites, not replacement clickable controls.
orbit=Image.new("RGBA",(1366*3,718*3),(0,0,0,0))
od=ImageDraw.Draw(orbit)
for bounds,rgba,thickness in (
    ((529,143,849,414),(8,134,255,55),3),
    ((541,152,842,404),(91,40,249,38),2),
    ((548,164,830,395),(18,100,241,18),2),
):
    od.ellipse(tuple(coord*3 for coord in bounds),outline=rgba,width=thickness*3)
orbit=orbit.resize((1366,718),Image.Resampling.LANCZOS)
out=Image.alpha_composite(out.convert("RGBA"),orbit.filter(ImageFilter.GaussianBlur(10)))
out=Image.alpha_composite(out,orbit).convert("RGB")
out.save(path,optimize=True)
print("GENERATED",path, path.stat().st_size,out.size)
