#!/usr/bin/env python3
"""Run the real voxel player on a private GPU display, capture or benchmark it."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import select
import signal
import subprocess
import sys
import tempfile
import time


def stop(process):
    if process is not None and process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--player', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--benchmark', action='store_true')
    parser.add_argument('--size', type=float, default=.25)
    parser.add_argument('--goat-target', type=int, choices=(0,1,2), default=1)
    parser.add_argument('--arguments-json', type=Path, help='Verified native launch argument array for the Modern voxel mode')
    parser.add_argument('--expect-native-death', action='store_true', help='Require a native-driven character death and nonempty physical debris')
    parser.add_argument('--diagnostic', choices=('no-goats','no-shadows','no-render'))
    parser.add_argument('--graphics-api', choices=('vulkan','glcore'), default='vulkan')
    parser.add_argument('--backend', choices=('xvfb','compositor'), default='xvfb')
    parser.add_argument('--width', type=int, default=1280)
    parser.add_argument('--height', type=int, default=960)
    args = parser.parse_args()
    if not args.player.is_file() or not os.access(args.player, os.X_OK):
        raise ValueError('Player must be an executable file')
    if not .2 <= args.size <= 1 or not all(320 <= v <= 3840 for v in (args.width, args.height)):
        raise ValueError('Invalid cell size or screen dimensions')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    runtime = Path(tempfile.mkdtemp(prefix='mc-voxel-',dir=f'/run/user/{os.getuid()}'))
    env = dict(os.environ)
    for name in ('DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'SESSION_MANAGER', 'XAUTHORITY'):
        env.pop(name, None)
    env.update(XDG_RUNTIME_DIR=str(runtime), XDG_CONFIG_HOME=str(runtime/'config'),
               XDG_CACHE_HOME=str(runtime/'cache'), XDG_SESSION_TYPE='x11', SDL_VIDEODRIVER='x11')
    alsa = runtime/'alsa.conf'
    alsa.write_text('pcm.!default { type null }\n', encoding='utf-8')
    env['ALSA_CONFIG_PATH'] = str(alsa)
    player = display = None
    binary = args.player.resolve()
    managed = binary.parent / (binary.stem + '_Data') / 'Managed/MagicCarpet.Native.Runtime.dll'
    native = binary.parent / (binary.stem + '_Data') / 'Plugins/x86_64/libsim_capi.so'
    def hashes():
        result = {}
        for path in (binary, managed, *([native] if args.arguments_json else [])):
            with path.open('rb') as stream:
                result[str(path)] = hashlib.file_digest(stream, 'sha256').hexdigest()
        return result
    receipt = {'schema':'mc-voxel-run/v1', 'actual_desktop':False, 'backend':args.backend, 'private_runtime':str(runtime), 'mode':'benchmark' if args.benchmark else 'offline capture', 'artifacts':hashes()}
    try:
        if args.backend == 'xvfb':
            read_fd, write_fd = os.pipe()
            try:
                with (output/'display.log').open('w', encoding='utf-8') as log:
                    display = subprocess.Popen(['Xvfb','-displayfd',str(write_fd),'-screen','0',f'{args.width}x{args.height}x24','-nolisten','tcp'],
                        pass_fds=(write_fd,), stdout=log, stderr=subprocess.STDOUT, start_new_session=True, env=env)
                os.close(write_fd)
                write_fd = None
                if not select.select([read_fd], [], [], 20)[0]:
                    raise TimeoutError('Private display did not start')
                number = os.read(read_fd,32).decode().strip()
                if not number.isdigit():
                    raise RuntimeError('Invalid display number')
                env['DISPLAY'] = ':' + number
            finally:
                os.close(read_fd)
                if write_fd is not None:
                    os.close(write_fd)
        command = [str(binary),'-force-'+args.graphics_api,'-screen-fullscreen','0','-screen-width',str(args.width),'-screen-height',str(args.height),
                   '-logFile',str(output/'player.log'),'-mc-voxel-size',str(args.size),
                   '-mc-voxel-goat-target',str(args.goat_target),
                   '-mc-voxel-benchmark' if args.benchmark else '-mc-voxel-capture', str(output/'result')]
        receipt['command'] = command
        if args.arguments_json:
            extra=json.loads(args.arguments_json.read_text(encoding='utf-8'))
            if not isinstance(extra,list) or not all(isinstance(x,str) for x in extra):
                raise ValueError('Native arguments must be a string array')
            command += extra
        if args.diagnostic:
            command += ['-mc-voxel-diagnostic', args.diagnostic]
        launch = command
        if args.backend == 'compositor':
            command_file=output/'player-command.json'
            command_file.write_text(json.dumps(command),encoding='utf-8')
            session=runtime/'session.py'
            session.write_text('#!'+sys.executable+'\nimport json,subprocess,sys\nfrom pathlib import Path\n'
                +'command=json.loads(Path('+repr(str(command_file))+').read_text())\n'
                +'sys.exit(subprocess.run(command).returncode)\n',encoding='utf-8')
            session.chmod(0o700)
            env.update(QT_QPA_PLATFORM='offscreen',XDG_SESSION_TYPE='wayland')
            launch=['dbus-run-session','--','kwin_wayland','--virtual','--width',str(args.width),'--height',str(args.height),
                    '--xwayland','--socket','voxel-display','--no-lockscreen','--no-global-shortcuts','--no-kactivities','--exit-with-session',str(session)]
        with (output/'stdout.log').open('w', encoding='utf-8') as log:
            player = subprocess.Popen(launch, stdout=log, stderr=subprocess.STDOUT, start_new_session=True, env=env)
        deadline = time.monotonic() + 900
        while player.poll() is None:
            if time.monotonic() > deadline:
                raise TimeoutError('Player exceeded 900 seconds')
            time.sleep(.5)
        if player.returncode != 0:
            raise RuntimeError(f'Player exit {player.returncode}; inspect player.log')
        report_path = output/'result/report.json'
        report = json.loads(report_path.read_text(encoding='utf-8'))
        character_scene=args.arguments_json and '-mc-character-follow' in command
        expected_impacts=3 if args.arguments_json else 4
        if args.arguments_json and report['nativeTick'] <= report['firstNativeTick']:
            raise RuntimeError('Native gameplay did not advance; inspect report.json')
        if args.arguments_json and report.get('belowTerrain',-1) != 0:
            raise RuntimeError('Active debris passed below its terrain contact surface')
        if not args.diagnostic and not character_scene and (report['impacts'] != expected_impacts or (not args.arguments_json and report['goatDeaths'] != 1) or report['removedCells'] <= 0):
            raise RuntimeError('Runtime did not complete its expected impacts and destruction; inspect report.json')
        if args.arguments_json and not character_scene and not args.diagnostic:
            if report.get('completedEdits') != 3 or report.get('editing', True) or report.get('groundRemoved', 0) <= 0 or report.get('houseRemoved', 0) <= 0 or report.get('craterDepth', 0) < .5:
                raise RuntimeError('House/ground edits or physical crater check failed; inspect report.json')
        if args.expect_native_death:
            log=(output/'player.log').read_text(errors='replace')
            slot=command[command.index('-mc-character-follow')+1] if character_scene else None
            deaths=re.findall(r'MC_VOXEL_NATIVE_DEATH tick=\d+ slot=(\d+) model=\d+ cells=([1-9]\d*)',log)
            if report['goatDeaths'] < 1 or report['sampled'] < 1 or not any(slot is None or found==slot for found,_ in deaths):
                raise RuntimeError('Expected native character death did not emit physical voxels')
        if receipt['artifacts'] != hashes():
            raise RuntimeError('Player changed during run')
        if not args.benchmark:
            video = output/'voxel-sandbox.mp4'
            subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-framerate','30','-i',str(output/'result/frames/frame-%05d.jpg'),
                            '-c:v','libx264','-crf','19','-pix_fmt','yuv420p','-movflags','+faststart',str(video)],check=True,timeout=300)
            receipt['video'] = str(video)
        receipt.update(status='complete', report=str(report_path))
    except Exception as error:
        receipt.update(status='error',message=str(error))
        raise
    finally:
        stop(player)
        stop(display)
        (output/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(receipt))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'status':'error','message':str(error)}),file=sys.stderr)
        sys.exit(1)
