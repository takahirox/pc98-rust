"""Serve only web/ at the Pages project path; root asset URLs must fail."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory

WEB = Path(__file__).resolve().parents[2] / 'web'

if __name__ == '__main__':
    with TemporaryDirectory() as directory:
        (Path(directory) / 'pc98-rust').symlink_to(WEB, target_is_directory=True)
        handler = partial(SimpleHTTPRequestHandler, directory=directory)
        ThreadingHTTPServer(('127.0.0.1', 8001), handler).serve_forever()
