"""The ``subgraphsearch`` command, installed with the package: the same program as the Rust binary."""

from __future__ import annotations

import signal
import sys


def main() -> None:
    from . import _core

    # let Ctrl-C stop a long search at once, as it would the Rust binary: Python's own handler
    # only runs between bytecodes, which never happens while the search holds the thread
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    sys.stdout.flush()
    sys.exit(_core.cli_main(["subgraphsearch", *sys.argv[1:]]))


if __name__ == "__main__":
    main()
