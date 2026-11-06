#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
# Keep resources as argument arrays, including owner paths containing spaces.
exec python3 - "$script_dir" "$@" <<'PY'
import os
from pathlib import Path
import sys
import tempfile

PROJECT = Path(sys.argv[1])
GENERATED = PROJECT.parent / 'generated'
HELP = """Usage: run-campaign.sh [campaign options] [Unity player arguments]

  --menu                  Open the campaign menu (default)
  --new-game              Start at the installed campaign's resolver-owned entry
  --load-save PATH        Resume a current runtime save (also --replay PATH)
  --save PATH             Save destination for F5 and the in-game Save action
  --package DIR           Install a package; repeat for the complete campaign set
  --owner-cache DIR       Native owner cache for the preceding package
  --presentation-cache DIR  Presentation cache for the preceding package
  --loading-cache DIR     Frontend/loading cache (required with explicit packages)
  --loading-assets DIR    Optional destination loading artwork cache override
  --menu-cache DIR        Optional original campaign-map/menu cache override
  --media-cache DIR       Optional original intro/frontend media cache override

An explicit package set replaces all defaults; Rust validates its semantic flow
and content identities. There is no fixed package count or canned entry replay.
The existing -mc-runtime-* resource/save options are also accepted. Paths are
relative to the current directory. Use -- before unparsed Unity arguments.
MC_CAMPAIGN_SAVE and MC_CAMPAIGN_REPLAY set save/load defaults; explicit options
take precedence. MC_WIDTH, MC_HEIGHT and MC_FULLSCREEN set the window defaults.
MC_PLAYER_PROJECT selects a separately qualified player's project; owner resources
still come from this launcher's canonical generated directory.
"""


class LaunchError(Exception):
    pass


def path_value(value, option):
    if not value.strip():
        raise LaunchError(f'{option} requires a nonblank path')
    return Path(value).absolute()


def directory(path, purpose):
    if not path.is_dir():
        raise LaunchError(f'Missing {purpose} directory: {path}. '
                          'Supply the installed owner resources with --package, '
                          '--owner-cache and --presentation-cache.')


def main(arguments):
    resources, player = [], []
    route, replay, save, loading, media = None, None, None, None, None
    menu_cache = None
    loading_assets = None
    aliases = {
        '--package': 'package', '-mc-runtime-package': 'package',
        '--owner-cache': 'owner', '-mc-runtime-owner-cache': 'owner',
        '--presentation-cache': 'presentation',
        '-mc-runtime-presentation-cache': 'presentation',
        '--loading-cache': 'loading', '-mc-runtime-loading-cache': 'loading',
        '--loading-assets': 'loading_assets', '-mc-runtime-loading-assets': 'loading_assets',
        '--menu-cache': 'menu', '-mc-runtime-menu-cache': 'menu',
        '--media-cache': 'media', '-mc-runtime-media-cache': 'media',
        '--save': 'save', '-mc-runtime-replay-save': 'save',
        '--load-save': 'replay', '--replay': 'replay',
        '-mc-runtime-replay-load': 'replay',
    }
    index = 0
    while index < len(arguments):
        option = arguments[index]
        index += 1
        if option == '--':
            if any(value == '-mc-game-runtime' or value.startswith('-mc-runtime-')
                   for value in arguments[index:]):
                raise LaunchError('Campaign options must precede --; only Unity '
                                  'player arguments may follow it')
            player.extend(arguments[index:])
            break
        if option in ('--help', '-h'):
            print(HELP)
            return
        if option == '--level':
            raise LaunchError('--level replay shortcuts have been removed. Use '
                              '--new-game for the campaign entry or --load-save PATH '
                              'to resume earned progress.')
        if option in ('--menu', '-mc-runtime-menu', '--new-game'):
            selected = 'new' if option == '--new-game' else 'menu'
            if route is not None:
                raise LaunchError('Choose only one of --menu, --new-game and --load-save')
            route = selected
            continue
        if option == '-mc-game-runtime':
            continue
        if option in aliases:
            if index >= len(arguments) or arguments[index].startswith('-'):
                raise LaunchError(f'{option} requires a path')
            value = path_value(arguments[index], option)
            index += 1
            kind = aliases[option]
            if kind == 'package':
                resources.append({'package': value})
            elif kind in ('owner', 'presentation'):
                if not resources:
                    raise LaunchError(f'{option} must follow a --package')
                if kind in resources[-1]:
                    raise LaunchError(f'Duplicate {kind} cache for {resources[-1]["package"]}')
                resources[-1][kind] = value
            elif kind == 'replay':
                if route is not None:
                    raise LaunchError('Choose only one of --menu, --new-game and --load-save')
                route, replay = 'replay', value
            elif kind == 'save':
                if save is not None:
                    raise LaunchError('Specify the save destination only once')
                save = value
            elif kind == 'loading':
                if loading is not None:
                    raise LaunchError('Specify the loading cache only once')
                loading = value
            elif kind == 'menu':
                if menu_cache is not None:
                    raise LaunchError('Specify the menu cache only once')
                menu_cache = value
            elif kind == 'loading_assets':
                if loading_assets is not None:
                    raise LaunchError('Specify the loading artwork cache only once')
                loading_assets = value
            else:
                if media is not None:
                    raise LaunchError('Specify the media cache only once')
                media = value
            continue
        if option.startswith('--') or option.startswith('-mc-runtime-'):
            raise LaunchError(f'Unknown campaign option: {option}. See --help.')
        player.append(option)

    if route is None:
        if os.environ.get('MC_CAMPAIGN_REPLAY'):
            replay = path_value(os.environ['MC_CAMPAIGN_REPLAY'], 'MC_CAMPAIGN_REPLAY')
            route = 'replay'
        else:
            route = 'menu'
    if save is None:
        save = path_value(os.environ.get('MC_CAMPAIGN_SAVE') or
                          str(GENERATED / 'campaign-save.json'), '--save')
    explicit_resources = bool(resources)
    if not resources:
        # Local installed owner content. A supplied set replaces this entire list.
        resources = [
            {'package': GENERATED / 'settlement-20261105-integration/owner-closed-v6/level1',
             'owner': GENERATED / 'settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level1',
             'presentation': GENERATED / 'settlement-20261105-integration/owner-closed-v6/night-presentation-v6'},
            {'package': GENERATED / 'mc-level2-vault-successor-closure-20261025',
             'owner': GENERATED / 'mc-b9s2-cursor-level2-vault-20261029',
             'presentation': GENERATED / 'level2-vault-successor-owner-cache-20261025'},
            {'package': GENERATED / 'settlement-20261105-integration/owner-closed-v6/level3',
             'owner': GENERATED / 'settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level3',
             'presentation': GENERATED / 'settlement-20261105-integration/owner-closed-v6/environment-complete/cache-level3'},
            {'package': GENERATED / 'mc-bhq6-ordinary-20261025-zclu5fjj/level4',
             'owner': GENERATED / 'mc-b9s2-cursor-level4-20261029',
             'presentation': GENERATED / 'mc-b9s2-cursor-level4-20261029'},
        ]
    seen = set()
    native = ['-mc-game-runtime']
    for resource in resources:
        package = resource['package']
        directory(package, 'campaign package')
        if not (package / 'manifest.json').is_file():
            raise LaunchError(f'Missing campaign package manifest: {package / "manifest.json"}')
        canonical = package.resolve()
        if canonical in seen:
            raise LaunchError(f'Duplicate campaign package: {package}')
        seen.add(canonical)
        native.extend(('-mc-runtime-package', str(package)))
        for kind in ('owner', 'presentation'):
            if kind in resource:
                directory(resource[kind], f'{kind} cache')
                native.extend((f'-mc-runtime-{kind}-cache', str(resource[kind])))
    if loading is None:
        if explicit_resources:
            raise LaunchError('An explicit package set requires --loading-cache DIR '
                              'for the campaign frontend and level transitions')
        loading = GENERATED / 'mc2-level1-night-cache-v2'
    if loading is not None:
        directory(loading, 'loading cache')
        native.extend(('-mc-runtime-loading-cache', str(loading)))
    if media is not None:
        directory(media, 'media cache')
        native.extend(('-mc-runtime-media-cache', str(media)))
    if loading_assets is not None:
        # The runtime owns missing-artwork recovery and retry for this selection.
        native.extend(('-mc-runtime-loading-assets', str(loading_assets)))
    if menu_cache is None and not explicit_resources:
        menu_cache = GENERATED / 'mc-shared-source-map-motion-20261026'
    if menu_cache is not None:
        directory(menu_cache, 'menu cache')
        native.extend(('-mc-runtime-menu-cache', str(menu_cache)))
    if replay is not None:
        if not replay.is_file():
            raise LaunchError(f'Missing runtime save: {replay}. Use --new-game to start a campaign.')
        native.extend(('-mc-runtime-replay-load', str(replay)))
    elif route == 'menu':
        native.append('-mc-runtime-menu')
    GENERATED.mkdir(parents=True, exist_ok=True)
    if not save.parent.is_dir() or save.is_dir():
        raise LaunchError(f'Invalid save destination: {save}. Its parent directory must exist.')
    native.extend(('-mc-runtime-replay-save', str(save)))
    run_dir = Path(tempfile.mkdtemp(prefix='campaign-play.', dir=GENERATED))
    print(f'Campaign log: {run_dir / "player.log"}', flush=True)
    print(f'Campaign save: {save}', flush=True)
    native.extend(('-screen-fullscreen', os.environ.get('MC_FULLSCREEN', '0'),
                   '-screen-width', os.environ.get('MC_WIDTH', '1600'),
                   '-screen-height', os.environ.get('MC_HEIGHT', '900'),
                   '-logFile', str(run_dir / 'player.log')))
    # No shell parsing: each caller-supplied path remains exactly one argument.
    original_project = PROJECT / 'Builds/Original'
    default_project = (original_project if original_project.exists() or original_project.is_symlink()
                       else PROJECT)
    player_project = Path(os.environ.get('MC_PLAYER_PROJECT') or str(default_project)).absolute()
    if not (player_project / 'run-player.sh').is_file():
        raise LaunchError(f'Missing qualified player launcher: {player_project / "run-player.sh"}')
    os.execv('/bin/sh', ['sh', str(player_project / 'run-player.sh'), *native, *player])


try:
    main(sys.argv[2:])
except (LaunchError, OSError) as error:
    print(f'Campaign launch failed: {error}', file=sys.stderr)
    sys.exit(64 if isinstance(error, LaunchError) else 1)
PY
