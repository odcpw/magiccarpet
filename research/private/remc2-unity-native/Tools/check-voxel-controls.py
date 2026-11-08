#!/usr/bin/env python3
"""Exercise real keyboard controls in the voxel player on an isolated X display."""
import argparse
import ctypes as C
import json
import os
import select
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from voxel_capture_support import inject, load_inputs, validate_receipts, validate_replay_save


def wait_for(predicate, seconds, message):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(.1)
    raise TimeoutError(message)


def session(args):
    output = args.output.resolve()
    log = output/'player.log'
    extra=json.loads(args.arguments_json.read_text()) if args.arguments_json else []
    if not isinstance(extra,list) or not all(isinstance(value,str) for value in extra):
        raise ValueError('Launch arguments must be a JSON string array')
    modern=args.modern or '-mc-voxel-modern' in extra
    save_path=None
    if args.expect_replay_save:
        if extra.count('-mc-runtime-replay-save')!=1:raise ValueError('Expected one explicit replay-save path')
        index=extra.index('-mc-runtime-replay-save')
        if index+1>=len(extra):raise ValueError('Missing replay-save path')
        save_path=Path(extra[index+1]).resolve()
        if save_path.exists():raise ValueError('Replay-save evidence path must be fresh')
    if modern and '-mc-voxel-receipts' not in extra:extra.append('-mc-voxel-receipts')
    player = subprocess.Popen([str(args.player.resolve()),'-force-vulkan','-screen-fullscreen','0',
                               '-screen-width','1280','-screen-height','960','-logFile',str(log),*extra])
    display = None
    try:
        ready='MC_VOXEL_MODERN_READY' if modern else 'MC_VOXEL_READY'
        def startup_ready():
            if not log.exists():return False
            text=log.read_text(errors='replace')
            if 'MC_FATAL ' in text or 'MC_RUNTIME_FATAL ' in text:raise RuntimeError('Player reported a fatal startup error; inspect player.log')
            return ready in text and (not modern or 'MC_GAME_RUNTIME_STARTUP VERIFIED' in text and 'MC_WATER_READY' in text)
        wait_for(startup_ready, 180, 'Player not ready')
        time.sleep(1)
        x = C.CDLL('libX11.so.6')
        xt = C.CDLL('libXtst.so.6')
        P, W = C.c_void_p, C.c_ulong
        x.XOpenDisplay.argtypes=[C.c_char_p]; x.XOpenDisplay.restype=P
        x.XDefaultRootWindow.argtypes=[P]; x.XDefaultRootWindow.restype=W
        x.XQueryTree.argtypes=[P,W,C.POINTER(W),C.POINTER(W),C.POINTER(C.POINTER(W)),C.POINTER(C.c_uint)]
        x.XFetchName.argtypes=[P,W,C.POINTER(C.c_char_p)]
        x.XGetGeometry.argtypes=[P,W,C.POINTER(W),C.POINTER(C.c_int),C.POINTER(C.c_int),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint),C.POINTER(C.c_uint)]
        x.XFree.argtypes=[P]
        x.XSetInputFocus.argtypes=[P,W,C.c_int,W]
        x.XStringToKeysym.argtypes=[C.c_char_p];x.XStringToKeysym.restype=W
        x.XKeysymToKeycode.argtypes=[P,W];x.XKeysymToKeycode.restype=C.c_uint
        x.XFlush.argtypes=[P];x.XCloseDisplay.argtypes=[P]
        xt.XTestFakeKeyEvent.argtypes=[P,C.c_uint,C.c_int,W]
        display=x.XOpenDisplay(os.environ['DISPLAY'].encode())
        if not display:
            raise RuntimeError('Cannot open private display')
        def find(window, depth=0):
            name=C.c_char_p()
            if x.XFetchName(display,window,C.byref(name)) and name.value:
                text=name.value.decode(errors='replace');x.XFree(name)
                if 'Magic Carpet' in text:
                    root=W();xx,yy=C.c_int(),C.c_int();ww,hh,border,depthValue=C.c_uint(),C.c_uint(),C.c_uint(),C.c_uint()
                    if x.XGetGeometry(display,window,C.byref(root),C.byref(xx),C.byref(yy),C.byref(ww),C.byref(hh),C.byref(border),C.byref(depthValue)) and ww.value>=320 and hh.value>=320:
                        return window
            if depth>=4:
                return None
            root,parent=W(),W();children=C.POINTER(W)();count=C.c_uint()
            if not x.XQueryTree(display,window,C.byref(root),C.byref(parent),C.byref(children),C.byref(count)):
                return None
            ids=[children[i] for i in range(count.value)]
            if children:x.XFree(children)
            for child in ids:
                result=find(child,depth+1)
                if result:return result
            return None
        window=wait_for(lambda:find(x.XDefaultRootWindow(display)),10,'Player window not found')
        x.XSetInputFocus(display,window,2,0);x.XFlush(display)
        def key(name,hold=.12):
            code=x.XKeysymToKeycode(display,x.XStringToKeysym(name.encode()))
            if not code:raise ValueError('Unknown key '+name)
            xt.XTestFakeKeyEvent(display,code,1,0);x.XFlush(display);time.sleep(hold)
            xt.XTestFakeKeyEvent(display,code,0,0);x.XFlush(display)
        def screenshot(name):
            subprocess.run(['import','-display',os.environ['DISPLAY'],'-window',str(window),str(output/(name+'.png'))],check=True,timeout=15)
        screenshot('initial')
        if modern:
            key('F2');wait_for(lambda:'MC_VOXEL_VIEW game' in log.read_text(errors='replace'),60,'F2 did not restore native game view')
            events=load_inputs(args.input_script) if args.input_script else [{'key':'w','hold':.7},{'key':'space'}]
            inject(os.environ['DISPLAY'],events,player.pid)
            (output/'native-input-events.json').write_text(json.dumps(events,indent=2)+'\n',encoding='utf-8')
            def evidence_ready():
                try:
                    return validate_receipts(log.read_text(errors='replace'),impact=args.expect_native_impact or args.input_script is None,
                        geometry=args.expect_native_geometry,visible=args.expect_visible_projectile,death=args.expect_native_death,
                        same_projectile=args.expect_same_projectile,target_slot=args.expect_target_slot,
                        ground=args.expect_native_ground,house_slot=args.expect_house_slot)
                except ValueError:
                    return None
            evidence=wait_for(evidence_ready,30,'Native voxel receipt or geometry convergence missing')
            (output/'native-evidence.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
            screenshot('native-result')
            key('F10');player.wait(timeout=10)
            text=log.read_text(errors='replace')
            if save_path is not None:validate_replay_save(text,save_path)
            if player.returncode or 'MC_VOXEL_MODERN_EXIT' not in text:
                raise RuntimeError('Native player did not exit cleanly')
            receipt={'schema':'mc-voxel-controls/v2','status':'passed','actual_desktop':False,
                     'inputs':['F2']+events+['F10'],'player':str(args.player.resolve()),
                     'native_evidence':str(output/'native-evidence.json')}
            (output/'controls.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
            print(json.dumps(receipt));return
        presets=[] if modern else [('3','0.25'),('1','0.5'),('2','0.3333333'),('r','0.3333333')]
        for i,(name,size) in enumerate(presets,1):
            key(name)
            wait_for(lambda:log.read_text(errors='replace').count('MC_VOXEL_RESET')>=i,30,'Reset key did not rebuild scene')
            resets=[line for line in log.read_text(errors='replace').splitlines() if line.startswith('MC_VOXEL_RESET')]
            if resets[-1] != 'MC_VOXEL_RESET cell='+size:
                raise RuntimeError('Preset key selected wrong voxel size: '+resets[-1])
            time.sleep(.75)
            screenshot('key-'+name)
        if modern:
            key('F2');wait_for(lambda:'MC_VOXEL_VIEW game' in log.read_text(errors='replace'),5,'F2 did not restore game view')
            key('b');wait_for(lambda:'MC_VOXEL_VIEW free' in log.read_text(errors='replace'),5,'B did not restore sandbox view')
        key('w',.7 if modern else .35)
        key('space')
        impact='MC_VOXEL_MODERN_IMPACT' if modern else 'MC_VOXEL_IMPACT'
        wait_for(lambda:impact in log.read_text(errors='replace'),20,'Space did not produce an impact')
        screenshot('impact')
        key('F10')
        player.wait(timeout=10)
        text=log.read_text(errors='replace')
        exit_marker='MC_VOXEL_MODERN_EXIT' if modern else 'MC_VOXEL_EXIT'
        unchanged='camera=(188.000, 60.500, 738.000)' if modern else 'MC_VOXEL_EXIT camera=(21.00, 9.00, -15.00)'
        if player.returncode or unchanged in text or exit_marker not in text:
            raise RuntimeError('Exit or movement check failed')
        receipt={'schema':'mc-voxel-controls/v1','status':'passed','actual_desktop':False,
                 'inputs':['F2','B','W','Space','F10'] if modern else ['3','1','2','R','W','Space','F10'],'reset_count':text.count('MC_VOXEL_RESET'),
                 'impact_count':text.count(impact),'player':str(args.player.resolve())}
        (output/'controls.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        print(json.dumps(receipt))
    finally:
        if display:x.XCloseDisplay(display)
        if player.poll() is None:
            player.terminate()
            try:player.wait(timeout=10)
            except subprocess.TimeoutExpired:
                player.kill();player.wait(timeout=5)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--player',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--arguments-json',type=Path)
    parser.add_argument('--modern',action='store_true',help='Check Modern controls when the launcher supplies its own native arguments')
    parser.add_argument('--input-script',type=Path,help='JSON array of native XTest key/relative motion events')
    parser.add_argument('--expect-replay-save',action='store_true')
    parser.add_argument('--expect-native-impact',action='store_true')
    parser.add_argument('--expect-native-geometry',action='store_true')
    parser.add_argument('--expect-native-ground',action='store_true')
    parser.add_argument('--expect-house-slot',type=int)
    parser.add_argument('--expect-visible-projectile',action='store_true')
    parser.add_argument('--expect-same-projectile',action='store_true')
    parser.add_argument('--expect-target-slot',type=int)
    parser.add_argument('--expect-native-death',action='store_true')
    parser.add_argument('--session',action='store_true',help=argparse.SUPPRESS)
    args=parser.parse_args()
    if args.session:
        session(args);return
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    runtime=Path(tempfile.mkdtemp(prefix='mc-voxel-controls-',dir=f'/run/user/{os.getuid()}'))
    env=dict(os.environ)
    for name in ('DISPLAY','WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS','SESSION_MANAGER','XAUTHORITY'):
        env.pop(name,None)
    env.update(XDG_RUNTIME_DIR=str(runtime),XDG_CONFIG_HOME=str(runtime/'config'),XDG_CACHE_HOME=str(runtime/'cache'),
               QT_QPA_PLATFORM='offscreen',XDG_SESSION_TYPE='wayland',SDL_VIDEODRIVER='x11')
    alsa=runtime/'alsa.conf';alsa.write_text('pcm.!default { type null }\n',encoding='utf-8');env['ALSA_CONFIG_PATH']=str(alsa)
    command=[sys.executable,str(Path(__file__).resolve()),'--session','--player',str(args.player.resolve()),'--output',str(args.output)]
    if args.arguments_json:command+=['--arguments-json',str(args.arguments_json.resolve())]
    if args.modern:command+=['--modern']
    if args.expect_house_slot is not None:
        if not 0 <= args.expect_house_slot <= 65535:raise ValueError('House slot must be a u16')
        command+=['--expect-house-slot',str(args.expect_house_slot)]
    if args.expect_target_slot is not None:
        if not 0 <= args.expect_target_slot <= 65535:raise ValueError('Target slot must be a u16')
        command+=['--expect-target-slot',str(args.expect_target_slot)]
    if args.input_script:command+=['--input-script',str(args.input_script.resolve())]
    for flag,enabled in (('--expect-replay-save',args.expect_replay_save),('--expect-native-impact',args.expect_native_impact),('--expect-native-geometry',args.expect_native_geometry),('--expect-native-ground',args.expect_native_ground),('--expect-visible-projectile',args.expect_visible_projectile),('--expect-same-projectile',args.expect_same_projectile),('--expect-native-death',args.expect_native_death)):
        if enabled:command.append(flag)
    read_fd,write_fd=os.pipe();display=None
    try:
        with (args.output/'display.log').open('w',encoding='utf-8') as log:
            display=subprocess.Popen(['Xvfb','-displayfd',str(write_fd),'-screen','0','1280x960x24','-nolisten','tcp'],
                                     pass_fds=(write_fd,),stdout=log,stderr=subprocess.STDOUT,env=env)
        os.close(write_fd);write_fd=None
        if not select.select([read_fd],[],[],20)[0]:raise TimeoutError('Private display did not start')
        number=os.read(read_fd,32).decode().strip()
        if not number.isdigit():raise RuntimeError('Invalid private display')
        env.update(DISPLAY=':'+number,XDG_SESSION_TYPE='x11')
        with (args.output/'session.log').open('w',encoding='utf-8') as log:
            subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=240)
    finally:
        os.close(read_fd)
        if write_fd is not None:os.close(write_fd)
        if display is not None and display.poll() is None:
            display.terminate();display.wait(timeout=10)
    receipt=json.loads((args.output/'controls.json').read_text(encoding='utf-8'))
    print(json.dumps(receipt))


if __name__=='__main__':
    try:main()
    except Exception as error:
        print(json.dumps({'status':'error','message':str(error)}),file=sys.stderr);sys.exit(1)
