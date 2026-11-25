#!/usr/bin/env python3
"""Serve one prepared WASM build to a local browser or the private tailnet."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class PlaytestFiles(SimpleHTTPRequestHandler):
    def __init__(self, *args, directory, allowed_hosts, **kwargs):
        self.allowed_hosts = allowed_hosts
        super().__init__(*args, directory=directory, **kwargs)

    def parse_request(self):
        if not super().parse_request():
            return False
        if self.headers.get('Host', '').lower() not in self.allowed_hosts:
            self.send_error(403, 'Invalid playtest host')
            return False
        return True

    def list_directory(self, _path):
        self.send_error(404)
        return None


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--host', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=8892)
    parser.add_argument('--allow-host', action='append', default=[])
    args = parser.parse_args()
    directory = args.directory.resolve(strict=True)
    if not (directory / 'original.html').is_file() or not (directory / 'original-api/sim_capi.wasm').is_file():
        parser.error('the selected directory does not contain a prepared playtest')
    allowed_hosts = {f'{args.host}:{args.port}'.lower(), *(name.lower() for name in args.allow_host)}
    handler = partial(PlaytestFiles, directory=str(directory), allowed_hosts=allowed_hosts)
    with ThreadingHTTPServer((args.host, args.port), handler) as server:
        server.serve_forever()
