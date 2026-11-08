"""Native voxel receipt validation and private-display keyboard injection."""
import ctypes as C
import json
import re
import time


def validate_receipts(log, *, impact=False, geometry=False, visible=False, death=False, same_projectile=False, target_slot=None, ground=False, house_slot=None):
    rows = []
    lines = log.splitlines()
    for line_number, line in enumerate(lines):
        marker = 'MC_VOXEL_RECEIPT '
        if marker not in line:
            continue
        parts = line.split(marker, 1)[1].split()
        try:
            row = dict(part.split('=', 1) for part in parts)
        except ValueError as exc:
            raise ValueError('Malformed native voxel receipt') from exc
        if len(row) != len(parts) or 'kind' not in row or 'generation' not in row:
            raise ValueError('Malformed native voxel receipt identity')
        row['_line'] = line_number
        rows.append(row)
    states = [r for r in rows if r['kind'] == 'state']
    if not states:
        raise ValueError('No native voxel geometry state receipt')
    try:
        generation = int(states[-1]['generation'])
        world_epoch = int(states[-1]['world_epoch'])
        restored = [r for r in states if int(r['world_epoch']) == world_epoch and r.get('restored') == '1']
        if not restored:
            raise ValueError('Missing initial native base/restoration receipt')
        start = restored[-1]['_line']
        current = [r for r in rows if r['_line'] >= start and r['kind'] in ('state','impact','projectile') and int(r['world_epoch']) == world_epoch]
        states = [r for r in current if r['kind'] == 'state']
        for row in states:
            for key in ('tick', 'revision', 'content_hash', 'native_removed', 'visual_removed', 'pending', 'restored', 'world_epoch', 'house_slot', 'native_ground', 'visual_ground', 'native_house', 'visual_house'):
                int(row[key])
        initial, final = states[0], states[-1]
        if initial['restored'] != '1':
            raise ValueError('Missing initial native base/restoration receipt')
        if int(final['native_ground']) + int(final['native_house']) != int(final['native_removed']) or int(final['visual_ground']) + int(final['visual_house']) != int(final['visual_removed']):
            raise ValueError('Voxel volume counts do not sum to their totals')
        if int(final['pending']) != 0 or int(final['native_removed']) != int(final['visual_removed']) or int(final['native_ground']) != int(final['visual_ground']) or int(final['native_house']) != int(final['visual_house']):
            raise ValueError('Native and visual geometry have not converged')
        impacts = [r for r in current if r['kind'] == 'impact']
        previous = 0
        for row in impacts:
            for key in ('tick', 'sequence', 'slot', 'slot_generation', 'target', 'has_target', 'revision', 'geometry_changed'):
                int(row[key])
            if int(row['sequence']) <= previous:
                raise ValueError('Repeated or out-of-order native impact receipt')
            previous = int(row['sequence'])
        projectiles = [r for r in current if r['kind'] == 'projectile']
        for row in projectiles:
            for key in ('tick', 'slot', 'slot_generation', 'source', 'model'):
                int(row[key])
        matches = [{'sequence': int(hit['sequence']), 'slot': int(hit['slot']),
                    'slot_generation': int(hit['slot_generation']),
                    'projectile_observed_tick': int(projectile['tick']), 'impact_observed_tick': int(hit['tick'])}
                   for hit in impacts for projectile in projectiles
                   if hit['slot'] == projectile['slot'] and hit['slot_generation'] == projectile['slot_generation']
                   and int(projectile['tick']) <= int(hit['tick'])]
        if int(initial['world_epoch']) != world_epoch:
            raise ValueError('Voxel world epoch changed within a projection')
        deaths = [r for r in rows if r['_line'] >= start and r['kind'] == 'death' and
                  int(r['generation']) == world_epoch and r.get('applied') == '1']
        death_lifetimes, death_sequences = set(), set()
        for row in deaths:
            for key in ('tick', 'sequence', 'slot', 'slot_generation', 'source'):
                int(row[key])
            identity = (world_epoch, int(row['slot']), int(row['slot_generation']), int(row['source']))
            sequence = int(row['sequence'])
            if identity in death_lifetimes or sequence in death_sequences:
                raise ValueError('Repeated applied native death lifetime or sequence')
            death_lifetimes.add(identity)
            death_sequences.add(sequence)
        if impact and not impacts:
            raise ValueError('No native impact receipt')
        if visible and not projectiles:
            raise ValueError('No visible native Fireball presentation')
        if same_projectile and not matches:
            raise ValueError('No visible native projectile lifetime matches an impact receipt')
        if target_slot is not None and not any(r['has_target'] == '1' and int(r['target']) == target_slot for r in impacts):
            raise ValueError('No native impact receipt for the requested target slot')
        if (geometry or ground or house_slot is not None) and (int(final['native_removed']) <= int(initial['native_removed']) or
                         not any(r['geometry_changed'] == '1' and int(r['revision']) <= int(final['revision']) for r in impacts)):
            raise ValueError('No completed native geometry change from an impact')
        if ground and int(final['native_ground']) <= int(initial['native_ground']):
            raise ValueError('No native terrain cells removed')
        if house_slot is not None and (int(final['house_slot']) != house_slot or int(initial['house_slot']) != house_slot or int(final['native_house']) <= int(initial['native_house'])):
            raise ValueError('No native cells removed from the requested house slot')
        emitted = []
        emitted_lifetimes = set()
        for match in re.finditer(r'MC_VOXEL_NATIVE_DEATH tick=(\d+) slot=(\d+) model=(\d+) cells=([1-9]\d*) generation=(\d+) slot_generation=(\d+) source=(\d+)', '\n'.join(lines[start:])):
            tick, slot, model, cells, gen, slot_generation, source = map(int, match.groups())
            if gen == world_epoch:
                identity = (gen, slot, slot_generation, source)
                if identity in emitted_lifetimes:
                    raise ValueError('Repeated native death fragment emission for the same lifetime')
                emitted_lifetimes.add(identity)
                emitted.append({'observed_tick': tick, 'slot': slot, 'model': model, 'cells': cells,
                                'generation': gen, 'slot_generation': slot_generation, 'source': source})
        if death and not any(int(row['slot']) == item['slot'] and int(row['slot_generation']) == item['slot_generation'] and
                             int(row['source']) == item['source'] for row in deaths for item in emitted):
            raise ValueError('No applied native death produced fragments for the same lifetime')
        def public(row):
            return {('observed_tick' if key == 'tick' else key):value for key,value in row.items() if key != '_line'}
        return {'generation': generation, 'world_epoch': world_epoch, 'tick_basis': 'Unity observation tick, not native event creation tick',
                'projectile_impact_match_basis': 'world epoch, slot and slot generation',
                'matched_projectile_impacts': matches, 'initial': public(initial), 'final': public(final),
                'impacts': list(map(public,impacts)), 'visible_projectiles': list(map(public,projectiles)),
                'applied_deaths': list(map(public,deaths)), 'emitted_deaths': emitted}
    except (KeyError, TypeError, ValueError) as exc:
        raise ValueError(f'Invalid native voxel evidence: {exc}') from exc


def validate_replay_save(log, path):
    if not path.is_file():
        raise ValueError('Expected fresh F5 replay save was not created')
    marker='MC_GAME_RUNTIME_REPLAY_SAVED path='+str(path)
    if not any(line.strip()==marker for line in log.splitlines()):
        raise ValueError('Player did not acknowledge the expected replay save')


def load_inputs(path):
    events = json.loads(path.read_text(encoding='utf-8'))
    if not isinstance(events, list) or len(events) > 1000:
        raise ValueError('Native input script must be a bounded array')
    duration = 0.0
    for event in events:
        if not isinstance(event, dict) or set(event) - {'key', 'hold', 'wait', 'wait_for', 'dx', 'dy'}:
            raise ValueError('Invalid native input event')
        if 'key' in event and (not isinstance(event['key'], str) or not event['key'] or len(event['key']) > 40):
            raise ValueError('Invalid X key name')
        if 'wait_for' in event and (not isinstance(event['wait_for'], str) or not event['wait_for'] or len(event['wait_for']) > 200):
            raise ValueError('Invalid native log condition')
        for field, maximum in (('hold', 10), ('wait', 60), ('dx', 2000), ('dy', 2000)):
            value = event.get(field, 0)
            if not isinstance(value, (int, float)) or isinstance(value, bool) or not -maximum <= value <= maximum:
                raise ValueError(f'Invalid native input {field}')
            if field in ('hold', 'wait') and value < 0:
                raise ValueError(f'Invalid native input {field}')
            if field in ('dx', 'dy') and int(value) != value:
                raise ValueError(f'Invalid native input {field}')
        duration += event.get('wait', 0) + (event.get('hold', .12) if 'key' in event else 0) + (15 if 'wait_for' in event else 0)
    if duration > 60:
        raise ValueError('Native input script exceeds 60 seconds')
    return events


def wait_and_inject(player, log_path, display_name, events, output_path, timeout=900, early=False):
    if not display_name:
        raise RuntimeError('Compositor did not provide an XWayland DISPLAY')
    deadline = time.monotonic() + timeout
    while player.poll() is None and time.monotonic() < deadline:
        if log_path.exists():
            text = log_path.read_text(errors='replace')
            if 'MC_FATAL ' in text or 'MC_RUNTIME_FATAL ' in text:
                raise RuntimeError('Player reported a fatal startup error; inspect player.log')
            if 'Selected window backend:' in text and 'Selected window backend: x11' not in text:
                raise RuntimeError('Native input requires the player X11 backend')
            if 'Selected window backend: x11' in text and 'MC_VOXEL_MODERN_READY' in text and 'MC_GAME_RUNTIME_STARTUP VERIFIED' in text and (early or 'MC_WATER_READY' in text):
                window = inject(display_name, events, player.pid, log_path)
                output_path.with_name('native-window.json').write_text(json.dumps(window,indent=2)+'\n',encoding='utf-8')
                output_path.write_text(json.dumps(events, indent=2)+'\n', encoding='utf-8')
                return
        time.sleep(.1)
    raise RuntimeError('Native voxel player exited or timed out before input')


def inject(display_name, events, expected_pid, log_path):
    """Inject XTest events; each event's waits run before its key or motion."""
    x=C.CDLL('libX11.so.6'); xt=C.CDLL('libXtst.so.6'); P,W=C.c_void_p,C.c_ulong
    x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=P
    x.XDefaultRootWindow.argtypes=[P];x.XDefaultRootWindow.restype=W
    x.XQueryTree.argtypes=[P,W,C.POINTER(W),C.POINTER(W),C.POINTER(C.POINTER(W)),C.POINTER(C.c_uint)]
    x.XFetchName.argtypes=[P,W,C.POINTER(C.c_char_p)];x.XFree.argtypes=[P]
    x.XInternAtom.argtypes=[P,C.c_char_p,C.c_int];x.XInternAtom.restype=W
    x.XGetWindowProperty.argtypes=[P,W,W,C.c_long,C.c_long,C.c_int,W,C.POINTER(W),C.POINTER(C.c_int),C.POINTER(W),C.POINTER(W),C.POINTER(C.POINTER(C.c_ubyte))]
    class ClassHint(C.Structure):
        _fields_=[('name',C.c_void_p),('class_name',C.c_void_p)]
    x.XGetClassHint.argtypes=[P,W,C.POINTER(ClassHint)]
    x.XSetInputFocus.argtypes=[P,W,C.c_int,W];x.XRaiseWindow.argtypes=[P,W]
    x.XGetInputFocus.argtypes=[P,C.POINTER(W),C.POINTER(C.c_int)];x.XSync.argtypes=[P,C.c_int]
    x.XStringToKeysym.argtypes=[C.c_char_p];x.XStringToKeysym.restype=W
    x.XKeysymToKeycode.argtypes=[P,W];x.XKeysymToKeycode.restype=C.c_uint
    x.XFlush.argtypes=[P];x.XCloseDisplay.argtypes=[P]
    xt.XTestFakeKeyEvent.argtypes=[P,C.c_uint,C.c_int,W]
    xt.XTestFakeRelativeMotionEvent.argtypes=[P,C.c_int,C.c_int,W]
    display=x.XOpenDisplay(display_name.encode())
    if not display:raise RuntimeError('Cannot open private X display')
    pid_atom=x.XInternAtom(display,b'_NET_WM_PID',1)
    def identity(window):
        actual=W();fmt=C.c_int();nitems=W();remaining=W();data=C.POINTER(C.c_ubyte)()
        status=x.XGetWindowProperty(display,window,pid_atom,0,1,0,0,C.byref(actual),C.byref(fmt),C.byref(nitems),C.byref(remaining),C.byref(data))
        try:
            pid=C.cast(data,C.POINTER(W))[0] if status==0 and data and fmt.value==32 and nitems.value==1 else None
        finally:
            if data:x.XFree(data)
        hint=ClassHint();wm_class=None
        if x.XGetClassHint(display,window,C.byref(hint)):
            try:
                wm_class=[C.string_at(value).decode(errors='replace') if value else '' for value in (hint.name,hint.class_name)]
            finally:
                if hint.name:x.XFree(hint.name)
                if hint.class_name:x.XFree(hint.class_name)
        return pid,wm_class
    def find(window, depth=0):
        name=C.c_char_p()
        if x.XFetchName(display,window,C.byref(name)) and name.value:
            text=name.value.decode(errors='replace');x.XFree(name)
            if 'Magic Carpet' in text:
                pid,wm_class=identity(window)
                if pid==expected_pid and wm_class and any(wm_class):return window,wm_class,text
        if depth>=4:return None
        root,parent=W(),W();children=C.POINTER(W)();count=C.c_uint()
        if not x.XQueryTree(display,window,C.byref(root),C.byref(parent),C.byref(children),C.byref(count)):return None
        ids=[children[i] for i in range(count.value)]
        if children:x.XFree(children)
        for child in ids:
            result=find(child,depth+1)
            if result:return result
        return None
    try:
        matched=find(x.XDefaultRootWindow(display))
        if not matched:raise RuntimeError('No titled X11 player window with matching PID and WM_CLASS')
        window,wm_class,title=matched
        def focus():
            x.XRaiseWindow(display,window);x.XSetInputFocus(display,window,2,0);x.XSync(display,0)
            focused=W();revert=C.c_int();x.XGetInputFocus(display,C.byref(focused),C.byref(revert))
            if focused.value!=window:raise RuntimeError('XWayland did not focus the player window')
        focus()
        for event in events:
            time.sleep(event.get('wait',0))
            if 'wait_for' in event:
                deadline=time.monotonic()+15
                while event['wait_for'] not in log_path.read_text(errors='replace'):
                    if time.monotonic()>=deadline:raise TimeoutError('Native input log condition was not observed')
                    time.sleep(.002)
            focus()
            if event.get('dx',0) or event.get('dy',0):
                if not xt.XTestFakeRelativeMotionEvent(display,int(event.get('dx',0)),int(event.get('dy',0)),0):
                    raise RuntimeError('XTest rejected pointer movement')
                x.XFlush(display)
            if 'key' in event:
                code=x.XKeysymToKeycode(display,x.XStringToKeysym(event['key'].encode()))
                if not code:raise ValueError('Unknown X key '+event['key'])
                if not xt.XTestFakeKeyEvent(display,code,1,0):raise RuntimeError('XTest rejected key press')
                x.XFlush(display);time.sleep(event.get('hold',.12))
                if not xt.XTestFakeKeyEvent(display,code,0,0):raise RuntimeError('XTest rejected key release')
                x.XFlush(display)
        return {'pid':expected_pid,'window':window,'wm_class':wm_class,'title':title,'display':display_name,'focused':True}
    finally:
        x.XCloseDisplay(display)
