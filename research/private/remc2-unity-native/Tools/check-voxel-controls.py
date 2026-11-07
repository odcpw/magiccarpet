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
    player = subprocess.Popen([str(args.player.resolve()),'-force-vulkan','-screen-fullscreen','0',
                               '-screen-width','1280','-screen-height','960','-logFile',str(log)])
    display = None
    try:
        wait_for(lambda: log.exists() and 'MC_VOXEL_READY' in log.read_text(errors='replace'), 60, 'Player not ready')
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
        for i,(name,size) in enumerate([('3','0.25'),('1','0.5'),('2','0.3333333'),('r','0.3333333')],1):
            key(name)
            wait_for(lambda:log.read_text(errors='replace').count('MC_VOXEL_RESET')>=i,30,'Reset key did not rebuild scene')
            resets=[line for line in log.read_text(errors='replace').splitlines() if line.startswith('MC_VOXEL_RESET')]
            if resets[-1] != 'MC_VOXEL_RESET cell='+size:
                raise RuntimeError('Preset key selected wrong voxel size: '+resets[-1])
            time.sleep(.75)
            screenshot('key-'+name)
        key('w',.35)
        key('space')
        wait_for(lambda:'MC_VOXEL_IMPACT' in log.read_text(errors='replace'),8,'Space did not produce an impact')
        screenshot('impact')
        key('F10')
        player.wait(timeout=10)
        text=log.read_text(errors='replace')
        if player.returncode or 'MC_VOXEL_EXIT camera=(21.00, 9.00, -15.00)' in text or 'MC_VOXEL_EXIT' not in text:
            raise RuntimeError('Exit or movement check failed')
        receipt={'schema':'mc-voxel-controls/v1','status':'passed','actual_desktop':False,
                 'inputs':['3','1','2','R','W','Space','F10'],'reset_count':text.count('MC_VOXEL_RESET'),
                 'impact_count':text.count('MC_VOXEL_IMPACT'),'player':str(args.player.resolve())}
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
            subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=180)
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
