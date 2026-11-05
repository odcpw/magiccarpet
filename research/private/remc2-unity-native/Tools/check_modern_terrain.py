"""Exact offline terrain channel check, independent of Unity pixel conversion.

uv run --offline --no-project --with pillow==13.0.0 python Tools/check_modern_terrain.py --self-test
Reads assets only; never regenerates or accepts changed output.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys
from PIL import Image, ImageChops, __version__

SCHEMA = 'mc-modern-terrain-pack-check/v1'

def compare(color, rough, packed):
    if color.size != rough.size or packed.size != color.size:
        return [{'code':'dimensions_mismatch','source':list(color.size),'roughness':list(rough.size),'packed':list(packed.size)}]
    errors = []
    expected = (*color.convert('RGB').split(), rough.convert('RGB').getchannel('R'))
    actual = packed.convert('RGBA').split()
    for channel, source, output in zip('RGBA', expected, actual):
        difference = ImageChops.difference(source, output)
        count = source.width * source.height - difference.histogram()[0]
        if count:
            y = difference.getbbox()[1]
            x = next(x for x in range(source.width) if difference.getpixel((x,y)))
            errors.append({'code':'channel_mismatch','channel':channel,'count':count,
                'first':{'x':x,'y':y,'source':source.getpixel((x,y)),'packed':output.getpixel((x,y))}})
    return errors

def negative_controls():
    color = Image.new('RGB',(2,2),(21,43,87))
    rough = Image.new('RGB',(2,2),(99,147,201))
    good = Image.merge('RGBA',(*color.split(),rough.getchannel('R')))
    if compare(color,rough,good):raise AssertionError('valid packing rejected')
    for channel in [0,3]:
        bad=good.copy();pixel=list(bad.getpixel((1,0)));pixel[channel]+=1;bad.putpixel((1,0),tuple(pixel))
        errors=compare(color,rough,bad)
        if len(errors)!=1 or errors[0]['channel']!='RGBA'[channel] or errors[0]['count']!=1:
            raise AssertionError('one-byte corruption was not identified exactly')
    return {'valid_accepted':True,'one_rgb_byte_rejected':True,'one_alpha_byte_rejected':True}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--asset-root',type=Path,default=Path(__file__).resolve().parent.parent/'Assets/Resources/ModernTextures')
    parser.add_argument('--self-test',action='store_true')
    args=parser.parse_args()
    if __version__!='13.0.0':raise RuntimeError('Use pinned Pillow13.0.0 for the independent decoder')
    result={'schema':SCHEMA,'status':'passed','pillow':__version__,'layers':[]}
    if args.self_test:result['negative_controls']=negative_controls()
    for layer in ['Rock','Ground']:
        paths=[args.asset_root/(layer+suffix) for suffix in ['Albedo.jpg','Roughness.jpg','AlbedoRoughness.png']]
        with Image.open(paths[0]) as color,Image.open(paths[1]) as rough,Image.open(paths[2]) as packed:
            errors=compare(color,rough,packed)
            result['layers'].append({'name':layer,'dimensions':list(packed.size),'errors':errors,
                'file_sha256':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}})
            if errors:result['status']='failed'
    print(json.dumps(result,sort_keys=True))
    return 0 if result['status']=='passed' else 1

if __name__=='__main__':
    try:sys.exit(main())
    except (OSError,RuntimeError,AssertionError) as error:
        print(json.dumps({'schema':SCHEMA,'status':'error','error':{'code':type(error).__name__,'message':str(error)}}),file=sys.stderr)
        sys.exit(2)
