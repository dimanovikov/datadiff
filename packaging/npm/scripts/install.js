// Fetches the release binary that matches this package's version and platform.
// The npm package carries no binary of its own, so publishing it never has to
// wait on a build.
const fs = require("node:fs");
const path = require("node:path");
const https = require("node:https");
const { execFileSync } = require("node:child_process");

const { version } = require("../package.json");
const TAG = `v${version}`;
const BASE = "https://github.com/dimanovikov/datadiff/releases/download";

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin.tar.gz",
  "linux-x64": "x86_64-unknown-linux-gnu.tar.gz",
  "linux-arm64": "aarch64-unknown-linux-gnu.tar.gz",
  "win32-x64": "x86_64-pc-windows-msvc.zip",
};

function download(url, dest, redirects = 0) {
  if (redirects > 5) return Promise.reject(new Error("too many redirects"));
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          resolve(download(res.headers.location, dest, redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`${url} returned ${res.statusCode}`));
          return;
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on("finish", () => file.close(resolve));
        file.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const key = `${process.platform}-${process.arch}`;
  const suffix = TARGETS[key];
  if (!suffix) {
    // Intel Macs have no prebuilt binary; cargo is the documented way there.
    throw new Error(
      `no prebuilt datadiff binary for ${key}. Install it with \`cargo install datadiff\` instead.`,
    );
  }

  const vendor = path.join(__dirname, "..", "bin");
  fs.mkdirSync(vendor, { recursive: true });
  const archive = path.join(vendor, suffix.endsWith(".zip") ? "d.zip" : "d.tar.gz");

  await download(`${BASE}/${TAG}/datadiff-${TAG}-${suffix}`, archive);
  // bsdtar ships with macOS, Windows 10+ and every Linux image we target, and
  // reads both .tar.gz and .zip, so one call covers all platforms.
  execFileSync("tar", ["-xf", archive, "-C", vendor], { stdio: "inherit" });
  fs.unlinkSync(archive);

  if (process.platform !== "win32") {
    fs.chmodSync(path.join(vendor, "datadiff"), 0o755);
  }
}

main().catch((err) => {
  console.error(`datadiff: ${err.message}`);
  process.exit(1);
});
