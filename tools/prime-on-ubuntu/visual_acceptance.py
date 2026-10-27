#!/usr/bin/env python3
"""Owner mockup versus live Wayland compositor: comparison, never self-approval."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageStat

OWNER_SIZE=(1672,941)
DESKTOP_TOP=581

def compare(reference,candidate,out):
    if not reference.is_file(): raise FileNotFoundError(f"Missing owner reference: {reference}")
    if not candidate.is_file(): raise FileNotFoundError(f"Missing compositor proof: {candidate}")
    with Image.open(reference) as src,Image.open(candidate) as actual:
        if src.width!=OWNER_SIZE[0] or src.height not in (582, OWNER_SIZE[1]):
            raise ValueError(f"Unexpected reference dimensions: {src.size}")
        reference_top=src.convert("RGB").crop((0,0,OWNER_SIZE[0],min(DESKTOP_TOP,src.height)))
        actual=actual.convert("RGB")
        # Preserve pixel geometry from the approved mockup's 1672 px-wide
        # desktop region. Its lower panels are explanatory states, not wallpaper.
        compare_height=min(actual.height, round(reference_top.height*actual.width/OWNER_SIZE[0]))
        wanted=reference_top.resize((actual.width,compare_height),Image.Resampling.LANCZOS)
        actual=actual.crop((0,0,actual.width,compare_height))
        diff=ImageChops.difference(wanted,actual)
        score=[round(x,2) for x in ImageStat.Stat(diff).mean]
        display=Image.new("RGB",(actual.width*2,actual.height+48),"#081426")
        display.paste(wanted,(0,48));display.paste(actual,(actual.width,48))
        draw=ImageDraw.Draw(display)
        draw.text((12,14),"OWNER REFERENCE",fill="white")
        draw.text((actual.width+12,14),"KRATOS LIVE COMPOSITOR",fill="white")
        out.mkdir(parents=True,exist_ok=True)
        display.save(out/"comparison.png")
        diff.save(out/"difference.png")
    result={"schema":"prime.visual-gate.v1","reference":str(reference),
            "candidate":str(candidate),"viewport":list(actual.size),
            "mean_difference_rgb":score,
            "comparison_region":"owner_main_desktop_only_uniform_horizontal_scale",
            "reference_contains_instructional_callouts":True,
            "exact_pixel_match_expected":False,
            "owner_visual_accepted":False,
            "system_accepted":False,"merge_permitted":False,
            "default_desktop_permitted":False,
            "note":"Raster metrics are diagnostic, not an approval. Require manual UI and functional acceptance."}
    (out/"visual-gate.json").write_text(json.dumps(result,indent=2)+"\n")
    return result

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--reference",type=Path,required=True)
    parser.add_argument("--candidate",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    print(json.dumps(compare(args.reference,args.candidate,args.output),indent=2))

if __name__=="__main__":main()
