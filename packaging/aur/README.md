# AUR packaging

`PKGBUILD` for the [`datadiff`](https://aur.archlinux.org/packages/datadiff)
AUR package. The AUR is its own git host, so publishing means pushing these
two files to it — this directory is the source of truth they are copied from.

First publish:

```sh
git clone ssh://aur@aur.archlinux.org/datadiff.git aur-datadiff
cp PKGBUILD .SRCINFO aur-datadiff/
cd aur-datadiff && git add -A && git commit -m "Initial import: datadiff 0.4.0" && git push
```

On each release bump `pkgver`, reset `pkgrel=1`, refresh `sha256sums` with the
tag tarball's checksum, regenerate `.SRCINFO` with `makepkg --printsrcinfo`,
and push again. Requires an AUR account with an SSH key registered.
