"""Reproduce owner-reference Prime system icons, without generic substitutes.

Only the supplied Prime reference top desktop screenshot is used. The
finished artwork remains separate from the XDG third-party icons.
"""
from pathlib import Path
from PIL import Image,ImageDraw
ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/"crates/prime-shell/assets/reference-icons/cosmic_scene_reference.webp"
DEST=ROOT/"crates/prime-shell/assets/reference-icons"
# Manually measured bounds, excluding the label below each icon.
BOUNDS={
 "machine":(637,73,730,158),
 "settings":(916,68,1015,161),
 "terminal":(542,160,637,241),
 "store":(1018,162,1112,246),
 "files":(505,275,593,361),
 "notes":(1054,274,1148,363),
 "music":(563,380,659,467),
 "gallery":(993,381,1090,466),
 "tools":(683,441,778,535),
 "chat":(881,440,977,532),
}
def build():
 source=Image.open(SOURCE).convert("RGB")
 assert source.size==(1672,582),source.size
 count=0
 for name,bounds in BOUNDS.items():
  tile=source.crop(bounds)
  if name in ("bin","projects"):
   # Standalone desktop objects preserve natural proportions, no fake square panel.
   import numpy as np
   original=np.asarray(tile.convert("RGB"),dtype=np.float32)
   peak=original.max(axis=2)
   # Opaque colored object; suppress the dark reference-wallpaper rectangle.
   a=np.clip((peak-31)*255/115,0,255).astype("uint8")
   item=tile.convert("RGBA")
   item.putalpha(Image.fromarray(a,"L"))
   item.thumbnail((88,85),Image.Resampling.LANCZOS)
   out=Image.new("RGBA",(96,96),(0,0,0,0))
   out.alpha_composite(item,((96-item.width)//2,(96-item.height)//2))
   tile=out
  else:
   tile=tile.resize((96,96),Image.Resampling.LANCZOS).convert("RGBA")
   mask=Image.new("L",(96,96),0); d=ImageDraw.Draw(mask)
   d.rounded_rectangle((1,1,94,94),radius=19,fill=255)
   tile.putalpha(mask)
  tile.save(DEST/(name+".png"),optimize=True)
  count+=1
 print("OWNER_REFERENCE_RETRIMMED_ICONS",count)
if __name__=="__main__":build()
