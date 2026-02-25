# Maintainer: Yakup Atahanov <yakup.atahanow.b@gmail.com>
pkgname=dmcp
pkgver=0.1.0
pkgrel=1
pkgdesc='MCP Manager - discover, manage, and invoke MCP servers'
url='https://github.com/YakupAtahanov/dmcp'
license=('GPL-3.0')
arch=('x86_64' 'i686' 'armv6h' 'armv7h' 'aarch64')
makedepends=('cargo')
depends=('gcc-libs')
optdepends=('polkit: for system-scope operations (pkexec)')
source=("$pkgname-$pkgver.tar.gz::$url/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
  cd "$pkgname-$pkgver"
  cargo build --release --locked
}

check() {
  cd "$pkgname-$pkgver"
  cargo test --release --locked
}

package() {
  cd "$pkgname-$pkgver"
  install -Dm755 target/release/dmcp "$pkgdir/usr/bin/dmcp"
  [[ -f man/dmcp.1 ]] && install -Dm644 man/dmcp.1 "$pkgdir/usr/share/man/man1/dmcp.1"
}
