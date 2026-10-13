"""List or extract files from X4 .cat/.dat archives.

Each .cat line is "<path> <size> <unix mtime> <md5>" (paths may contain spaces).
The matching .dat holds the file bodies back to back, in .cat order.

    python -I tools/x4cat.py list    <file.cat> [glob ...]
    python -I tools/x4cat.py extract <file.cat> <outdir> [glob ...]
"""
import argparse
import fnmatch
import hashlib
import os
import sys


def read_cat(cat_path):
    offset = 0
    with open(cat_path, encoding="utf-8") as f:
        for line in f:
            path, size, mtime, md5 = line.rstrip("\r\n").rsplit(" ", 3)
            size = int(size)
            yield path, offset, size, int(mtime), md5
            offset += size


def selected(cat_path, patterns):
    for entry in read_cat(cat_path):
        if not patterns or any(fnmatch.fnmatch(entry[0], p) for p in patterns):
            yield entry


def cmd_list(args):
    for path, _, size, _, _ in selected(args.cat, args.patterns):
        print(f"{size:>12} {path}")


def cmd_extract(args):
    dat_path = args.cat[:-4] + ".dat"
    outdir = os.path.abspath(args.outdir)
    count = bad = 0
    with open(dat_path, "rb") as dat:
        for path, offset, size, _, md5 in selected(args.cat, args.patterns):
            target = os.path.abspath(os.path.join(outdir, path))
            if not target.startswith(outdir + os.sep):
                sys.exit(f"refusing path outside outdir: {path}")
            dat.seek(offset)
            data = dat.read(size)
            if size and hashlib.md5(data).hexdigest() != md5:
                print(f"MD5 MISMATCH {path}", file=sys.stderr)
                bad += 1
            os.makedirs(os.path.dirname(target), exist_ok=True)
            with open(target, "wb") as out:
                out.write(data)
            count += 1
    print(f"extracted {count} files, {bad} md5 mismatches")
    sys.exit(1 if bad else 0)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("list")
    p.add_argument("cat")
    p.add_argument("patterns", nargs="*")
    p.set_defaults(func=cmd_list)
    p = sub.add_parser("extract")
    p.add_argument("cat")
    p.add_argument("outdir")
    p.add_argument("patterns", nargs="*")
    p.set_defaults(func=cmd_extract)
    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
