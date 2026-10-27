"""Make a layered, owner-reference cosmic wallpaper for the isolated Ubuntu preview.

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
d.ellipse((317,-2,1347,588),fill=255)
# Force-remove every flattened launcher search/text fragment in the central
# reference; the clean Menu-closed emblem is composited as a separate asset.
# A soft mask edge preserves the surrounding star field.
d.rounded_rectangle((548,75,1118,545),radius=70,fill=255)
# The flattened top mockup has app labels and search glass across the emblem.
# Remove its entire center; the logo is restored below from the separate
# owner-supplied Menu-closed reference art, never from placeholder geometry.
d.ellipse((665,136,999,444),fill=255)
d.rectangle((0,0,95,581),fill=255)
d.rectangle((1472,0,1671,232),fill=255)
d.rectangle((168,449,454,581),fill=255)
d.rectangle((1231,447,1541,581),fill=255)
d.rectangle((372,313,552,549),fill=255)
d.rectangle((1116,311,1306,550),fill=255)
d.rectangle((657,308,1019,354),fill=255)
mask=mask.filter(ImageFilter.MaxFilter(19)).filter(ImageFilter.GaussianBlur(13))
strength=np.asarray(mask).astype(np.float32)/255.0
# The flattened mockup embeds text *inside* its central logo preservation
# ellipse. Remove those remnants without erasing the luminous A/ring.
# Apply full alpha to actual unwanted glyph/search interiors. The broadened
# outline remains feathered by the original mask above.
strong=Image.new("L",(w,h),0)
glyphs=ImageDraw.Draw(strong)
for bounds in [
    (755,111,915,173),   # flattened Browser name, NOT the logo tip
    (642,171,745,225),   # Machine name near left ring
    (931,166,1056,226),  # Settings name near right ring
    (644,299,1032,362),  # duplicate search glass and lettering
    (613,453,762,504),   # menu labels near lower-left ring
    (878,451,1075,512),  # menu labels near lower-right ring
]:
    glyphs.rectangle(bounds,fill=255)
strong=strong.filter(ImageFilter.GaussianBlur(3))
strength=np.maximum(strength,np.asarray(strong).astype(np.float32)/255.0)
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
emblem=emblem.resize((300,273),Image.Resampling.LANCZOS)
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
layer.alpha_composite(emblem,(533,105))
upper=layer.convert("RGB")
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
out.save(path,optimize=True)
print("GENERATED",path, path.stat().st_size,out.size)
