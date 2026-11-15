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
  --authored-workspace DIR  Retained authored workspace for the preceding package
  --loading-cache DIR     Frontend/loading cache (required with explicit packages)
  --loading-assets DIR    Optional destination loading artwork cache override
  --menu-cache DIR        Optional original campaign-map/menu cache override
  --media-cache DIR       Optional original intro, cutscene and credits cache override
  --speech-cache DIR      Optional original campaign speech cache override

The default is the installed Original campaign (25 ordinary and 5 secret
missions). An explicit package set replaces all defaults; Rust validates its
semantic flow and content identities. No canned entry replay is required.
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


def default_campaign_resources():
    ordinary = 'mc-bhq6-ordinary-20261025-zclu5fjj'
    settlement = 'settlement-20261105-integration/owner-closed-v6'
    environments = 'campaign-original-environments'
    packages = {
        1: f'{settlement}/level1',
        2: 'mc-level2-vault-successor-closure-20261025',
        3: f'{settlement}/level3',
        5: 'dragon-bee-20261105-integration/level5-owner-current/level5',
        7: 'settlement-20261105-integration/owner-collision-closure-v4/level7',
        8: 'ul-buthnen-source-closure-20261114/level8',
        9: 'ribbon-20261104-integration/group-zero-owner-package-v4/level9',
        10: 'ribbon-20261104-integration/current-owner-package/level10',
        11: 'tropolos-source-closure-20261113/level11',
        12: 'archer-20261105-integration/level12-owner-current/level12',
        13: 'volcano-quake-20261104-integration/current-owner-package/level13',
        16: 'darklava-source-closure-20261114/level16',
        17: f'{environments}/level17-package',
        18: 'all-spells-20261104-integration/current-owner-package/level18',
        20: 'ammyridia-source-closure-20261114/level20',
        22: 'volcano-quake-20261104-integration/current-owner-package/level22',
        23: 'creatures-20261104-integration/current-owner-package/level23',
        24: 'ribbon-20261104-integration/current-owner-package/level24',
        25: 'mc-bhq10-owner-v3-level25-20261029',
    }
    owners = {
        1: f'{settlement}/environment-complete/cache-level1',
        2: 'mc-b9s2-cursor-level2-vault-20261029',
        3: f'{settlement}/environment-complete/cache-level3',
        4: 'mc-b9s2-cursor-level4-20261029',
        5: 'secret-lifecycle-20261111-source4-environment',
        7: f'{environments}/level7-complete',
        8: 'ul-buthnen-source-closure-20261114/environment-qualified',
        11: 'tropolos-source-closure-20261113/environment',
        16: 'darklava-source-closure-20261114/environment',
        17: f'{environments}/level17-complete',
        20: 'ammyridia-source-closure-20261114/environment',
        25: 'dragon-bee-20261105-integration/current-level25-environment',
    }
    presentations = {
        1: f'{settlement}/night-presentation-v6',
        2: 'level2-vault-successor-owner-cache-20261025',
    }
    for level in range(1, 26):
        owner = owners.get(level, f'{environments}/level{level}')
        yield {
            'package': GENERATED / packages.get(level, f'{ordinary}/level{level}'),
            'owner': GENERATED / owner,
            'presentation': GENERATED / presentations.get(level, owner),
        }

    secret_environment = GENERATED / 'mc-paj2-environment-source30-audio-20261026'
    yield {
        'package': GENERATED / 'mc-paj2-source30-campaign-package',
        'owner': secret_environment,
        'presentation': secret_environment,
    }
    for source in range(31, 35):
        environment = GENERATED / environments / f'source{source}'
        yield {
            'package': GENERATED / f'mc-paj2-source-secret-{source}-campaign-package',
            'owner': environment,
            'presentation': environment,
        }


def main(arguments):
    resources, player = [], []
    route, replay, save, loading, media, speech = None, None, None, None, None, None
    menu_cache = None
    loading_assets = None
    aliases = {
        '--package': 'package', '-mc-runtime-package': 'package',
        '--owner-cache': 'owner', '-mc-runtime-owner-cache': 'owner',
        '--presentation-cache': 'presentation',
        '-mc-runtime-presentation-cache': 'presentation',
        '--authored-workspace': 'workspace',
        '-mc-runtime-authored-workspace': 'workspace',
        '--loading-cache': 'loading', '-mc-runtime-loading-cache': 'loading',
        '--loading-assets': 'loading_assets', '-mc-runtime-loading-assets': 'loading_assets',
        '--menu-cache': 'menu', '-mc-runtime-menu-cache': 'menu',
        '--media-cache': 'media', '-mc-runtime-media-cache': 'media',
        '--speech-cache': 'speech', '-mc-runtime-speech-cache': 'speech',
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
            elif kind in ('owner', 'presentation', 'workspace'):
                if not resources:
                    raise LaunchError(f'{option} must follow a --package')
                if kind in resources[-1]:
                    raise LaunchError(f'Duplicate {kind} resource for {resources[-1]["package"]}')
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
            elif kind == 'speech':
                if speech is not None:
                    raise LaunchError('Specify the speech cache only once')
                speech = value
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
        resources = default_campaign_resources()
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
        if 'workspace' in resource:
            directory(resource['workspace'], 'authored workspace')
            native.extend(('-mc-runtime-authored-workspace', str(resource['workspace'])))
    if loading is None:
        if explicit_resources:
            raise LaunchError('An explicit package set requires --loading-cache DIR '
                              'for the campaign frontend and level transitions')
        loading = GENERATED / 'mc2-level1-night-cache-v2'
    if loading is not None:
        directory(loading, 'loading cache')
        native.extend(('-mc-runtime-loading-cache', str(loading)))
    if media is None:
        installed_media = GENERATED / 'mc2-campaign-media-cache-decoded'
        if installed_media.is_dir():
            media = installed_media
    if media is not None:
        directory(media, 'media cache')
        native.extend(('-mc-runtime-media-cache', str(media)))
    if speech is None:
        installed_speech = GENERATED / 'mc2-campaign-speech-current'
        if installed_speech.is_dir():
            speech = installed_speech
    if speech is not None:
        directory(speech, 'speech cache')
        native.extend(('-mc-runtime-speech-cache', str(speech)))
    if loading_assets is not None:
        # The runtime owns missing-artwork recovery and retry for this selection.
        native.extend(('-mc-runtime-loading-assets', str(loading_assets)))
    if menu_cache is None and not explicit_resources:
        menu_cache = (GENERATED / 'mc2-campaign-menu-cache-current').resolve()
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


if __name__ == '__main__':
    try:
        main(sys.argv[2:])
    except (LaunchError, OSError) as error:
        print(f'Campaign launch failed: {error}', file=sys.stderr)
        sys.exit(64 if isinstance(error, LaunchError) else 1)
PY
