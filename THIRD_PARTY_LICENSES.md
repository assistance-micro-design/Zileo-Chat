# Third-Party Licenses

This file lists all third-party dependencies used by Zileo Chat and their respective licenses,
as required by the Apache License 2.0.

Generated: 2025-12-11

## Summary by License Type

| License | Count | Notes |
|---------|-------|-------|
| MIT | ~180 | Permissive |
| Apache-2.0 | ~150 | Permissive |
| MIT OR Apache-2.0 | ~200 | Dual-licensed (permissive) |
| BSD-3-Clause | ~15 | Permissive |
| BSD-2-Clause | ~5 | Permissive |
| ISC | ~10 | Permissive |
| MPL-2.0 | ~5 | Weak copyleft |
| Unicode-3.0 | ~20 | Permissive (ICU data) |
| Unlicense | ~5 | Public domain equivalent |
| 0BSD | ~2 | Public domain equivalent |
| CC0-1.0 | ~3 | Public domain |
| Zlib | ~3 | Permissive |
| BSL-1.1 | 1 | SurrealDB (see note below) |

### Special License Notes

- **SurrealDB**: Licensed under BSL 1.1 (Business Source License). This is a source-available license that converts to Apache-2.0 after 4 years. Usage in Zileo Chat is permitted under BSL 1.1 terms.
- **MPL-2.0 dependencies** (cssparser, selectors): File-level copyleft - modifications to these specific files must be shared, but the rest of the codebase is unaffected.

---

## Node.js Dependencies (Frontend)

### Direct Dependencies

| Package | Version | License | Repository |
|---------|---------|---------|------------|
| @lucide/svelte | 0.560.0 | ISC | https://github.com/lucide-icons/lucide |
| @tauri-apps/api | 2.9.1 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| @tauri-apps/plugin-dialog | 2.4.2 | MIT OR Apache-2.0 | https://github.com/tauri-apps/plugins-workspace |
| zod | 4.1.13 | MIT | https://github.com/colinhacks/zod |

### Dev Dependencies

| Package | Version | License | Repository |
|---------|---------|---------|------------|
| @eslint/js | 9.39.1 | MIT | https://github.com/eslint/eslint |
| @humanspeak/svelte-virtual-list | 0.3.6 | MIT | https://github.com/nickreese/svelte-virtual-list |
| @playwright/test | 1.57.0 | Apache-2.0 | https://github.com/microsoft/playwright |
| @sveltejs/adapter-static | 3.0.10 | MIT | https://github.com/sveltejs/kit |
| @sveltejs/kit | 2.49.1 | MIT | https://github.com/sveltejs/kit |
| @sveltejs/vite-plugin-svelte | 6.2.1 | MIT | https://github.com/sveltejs/vite-plugin-svelte |
| @tauri-apps/cli | 2.9.5 | MIT OR Apache-2.0 | https://github.com/tauri-apps/tauri |
| @typescript-eslint/eslint-plugin | 8.48.1 | MIT | https://github.com/typescript-eslint/typescript-eslint |
| @typescript-eslint/parser | 8.48.1 | MIT | https://github.com/typescript-eslint/typescript-eslint |
| eslint | 9.39.1 | MIT | https://github.com/eslint/eslint |
| eslint-plugin-svelte | 2.46.0 | MIT | https://github.com/sveltejs/eslint-plugin-svelte |
| globals | 16.5.0 | MIT | https://github.com/sindresorhus/globals |
| jsdom | 27.2.0 | MIT | https://github.com/jsdom/jsdom |
| svelte | 5.45.6 | MIT | https://github.com/sveltejs/svelte |
| svelte-check | 4.0.0 | MIT | https://github.com/sveltejs/language-tools |
| typescript | 5.9.3 | Apache-2.0 | https://github.com/microsoft/TypeScript |
| typescript-eslint | 8.48.0 | MIT | https://github.com/typescript-eslint/typescript-eslint |
| vite | 7.2.6 | MIT | https://github.com/vitejs/vite |
| vitest | 4.0.15 | MIT | https://github.com/vitest-dev/vitest |

### Transitive Dependencies (Notable)

| Package | License | Notes |
|---------|---------|-------|
| acorn | MIT | JavaScript parser |
| cookie | MIT | Cookie parsing |
| debug | MIT | Debugging utility |
| devalue | MIT | Value serialization |
| esm-env | MIT | ESM environment detection |
| kleur | MIT | Terminal colors |
| magic-string | MIT | String manipulation |
| mrmime | MIT | MIME type detection |
| playwright-core | Apache-2.0 | Browser automation |

---

## Rust Dependencies (Backend)

### Direct Dependencies

| Crate | Version | License | Repository |
|-------|---------|---------|------------|
| tauri | 2.9.3 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |
| tauri-plugin-opener | 2.5.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| tauri-plugin-dialog | 2.4.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/plugins-workspace |
| serde | 1.0.228 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.145 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| tokio | 1.48.0 | MIT | https://github.com/tokio-rs/tokio |
| surrealdb | 2.4.0 | BSL-1.1 | https://github.com/surrealdb/surrealdb |
| anyhow | 1.0.100 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| thiserror | 1.0.69 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| tracing | 0.1.41 | MIT | https://github.com/tokio-rs/tracing |
| tracing-subscriber | 0.3.20 | MIT | https://github.com/tokio-rs/tracing |
| uuid | 1.18.1 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| chrono | 0.4.42 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono |
| async-trait | 0.1.89 | MIT OR Apache-2.0 | https://github.com/dtolnay/async-trait |
| futures | 0.3.31 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| regex | 1.12.2 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| once_cell | 1.21.3 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| tokio-util | 0.7.17 | MIT | https://github.com/tokio-rs/tokio |
| rig-core | 0.24.0 | MIT | https://github.com/0xPlaygrounds/rig |
| reqwest | 0.12.24 | MIT OR Apache-2.0 | https://github.com/seanmonstar/reqwest |
| futures-util | 0.3.31 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| keyring | 2.3.3 | MIT OR Apache-2.0 | https://github.com/hwchen/keyring-rs |
| aes-gcm | 0.10.3 | Apache-2.0 OR MIT | https://github.com/RustCrypto/AEADs |

### Build Dependencies

| Crate | Version | License | Repository |
|-------|---------|---------|------------|
| tauri-build | 2.5.2 | Apache-2.0 OR MIT | https://github.com/tauri-apps/tauri |

### Dev Dependencies

| Crate | Version | License | Repository |
|-------|---------|---------|------------|
| tempfile | 3.23.0 | MIT OR Apache-2.0 | https://github.com/Stebalien/tempfile |

### Transitive Dependencies by License

#### MIT License

```
async-stream, atk, atk-sys, base64, bcrypt, bincode, bytes, cairo-rs, cairo-sys-rs,
cargo_metadata, castaway, convert_case, darling, darling_core, darling_macro, dashmap,
deluxe, deluxe-core, deluxe-macros, derivative, dirs-next, dirs-sys-next, dmp, gdk,
gdk-pixbuf, gdk-pixbuf-sys, gdk-sys, gdkwayland-sys, gdkx11, gdkx11-sys, generic-array,
gio, gio-sys, glib, glib-macros, glib-sys, gobject-sys, gtk, gtk-sys, gtk3-macros,
h2, hmac, http-body, http-body-util, hyper, hyper-tls, hyper-util, javascriptcore-rs,
javascriptcore-rs-sys, memoffset, native-tls, nix, nom, openssl-sys, pango, pango-sys,
parking_lot, parking_lot_core, phf, phf_codegen, phf_generator, phf_macros, phf_shared,
rmp, rmp-serde, rmpv, rust_decimal, schemars, schemars_derive, secret-service, shlex,
soup3, soup3-sys, strsim, strum, strum_macros, synstructure, tokio, tokio-macros,
tokio-native-tls, tokio-tungstenite, tokio-util, tower, tower-http, tower-layer,
tower-service, tracing, tracing-attributes, tracing-core, tracing-futures, tracing-log,
tracing-serde, tracing-subscriber, urlpattern, webkit2gtk, webkit2gtk-sys, winnow,
x11, x11-dl, xdg-home, zbus, zbus_macros, zbus_names, zvariant, zvariant_derive,
zvariant_utils
```

#### Apache-2.0 License

```
approx, ar_archive_writer, cedar-policy, cedar-policy-core, cedar-policy-validator,
ciborium, ciborium-io, ciborium-ll, miette, miette-derive, openssl, revision,
revision-derive, storekey, sync_wrapper, trice, vart
```

#### MIT OR Apache-2.0 (Dual Licensed)

```
addr, aead, aes, aes-gcm, affinitypool, ahash, allocator-api2, ammonia, anyhow,
argon2, arrayvec, as-any, ascii-canvas, async-broadcast, async-channel, async-executor,
async-fs, async-graphql, async-graphql-derive, async-graphql-parser, async-graphql-value,
async-io, async-lock, async-process, async-recursion, async-signal, async-task,
async-trait, atomic-waker, autocfg, base64ct, bitflags, bit-set, bit-vec, blake2,
block-buffer, blocking, block-padding, blowfish, bytecount, bytemuck, bzip2-sys,
camino, cargo-platform, cargo_toml, cbc, cc, cfg-expr, cfg-if, chrono, cipher,
concurrent-queue, cookie, cpufeatures, crc32fast, crossbeam-channel, crossbeam-deque,
crossbeam-epoch, crossbeam-utils, crypto-common, ctor, ctr, deranged, digest,
dirs, dirs-sys, displaydoc, dyn-clone, either, embed-resource, ena, enumflags2,
enumflags2_derive, equivalent, erased-serde, event-listener, event-listener-strategy,
eventsource-stream, fastrand, fdeflate, field-offset, find-msvc-tools, fixedbitset,
flate2, fnv, form_urlencoded, futures, futures-channel, futures-core, futures-executor,
futures-io, futures-lite, futures-macro, futures-sink, futures-task, futures-timer,
futures-util, fxhash, geo, geo-types, getrandom, ghash, glob, hash32, hashbrown,
heapless, heck, hex, hkdf, hmac, html5ever, http, httparse, humantime, iana-time-zone,
idna, idna_adapter, indexmap, indoc, inout, ipnet, iri-string, is-docker, is-wsl,
itertools, itoa, jobserver, json-patch, jsonptr, keyboard-types, keyring, lalrpop,
lalrpop-util, lazy_static, lexicmp, libc, libz-sys, linux-keyutils, lock_api, log,
markup5ever, match_token, md-5, memchr, mime, minimal-lexical, miniz_oxide, muda,
ndarray, nom_locate, num, num-bigint, num-complex, num-conv, num_cpus, num-integer,
num-iter, num-rational, num-traits, once_cell, opaque-debug, ordered-stream, parking,
password-hash, paste, path-clean, pbkdf2, percent-encoding, pest, petgraph, pin-project,
pin-project-internal, pin-project-lite, pin-utils, piper, pkg-config, png, polling,
polyval, powerfmt, ppv-lite86, proc-macro2, proc-macro-crate, proc-macro-error,
proc-macro-error-attr, proc-macro-hack, psm, quote, rand, rand_chacha, rand_core,
raw-window-handle, rayon, rayon-core, ref-cast, ref-cast-impl, regex, regex-automata,
regex-syntax, reqwest, robust, roaring, rstar, rustc-hash, rustc_lexer, rustc_version,
rustix, rustls-pki-types, rustversion, salsa20, scopeguard, semver, serde,
serde_core, serde_derive, serde_derive_internals, serde_json, serde_repr,
serde_spanned, serde-untagged, serde_with, serde_with_macros, serialize-to-javascript,
serialize-to-javascript-impl, servo_arc, sha1, sha2, sharded-slab, signal-hook-registry,
simd-adler32, siphasher, slab, smallvec, smol_str, socket2, spade, spin, stable_deref_trait,
stacker, static_assertions, static_assertions_next, string_cache, string_cache_codegen,
syn, system-deps, tao, tauri, tauri-build, tauri-codegen, tauri-macros, tauri-plugin,
tauri-plugin-dialog, tauri-plugin-fs, tauri-plugin-opener, tauri-runtime,
tauri-runtime-wry, tauri-utils, tempfile, thiserror, thiserror-impl, thread_local,
time, time-core, time-macros, tinyvec, tinyvec_macros, tokio-rustls, toml,
toml_datetime, toml_edit, toml_parser, toml_writer, tungstenite, typeid, typenum,
ucd-trie, ulid, unicase, unicode-bidi, unicode-normalization, unicode-script,
unicode-segmentation, unicode-width, unicode-xid, universal-hash, url, utf-8,
utf8_iter, uuid, waker-fn, web_atoms, weezl, wry, zerocopy, zerocopy-derive, zeroize
```

#### BSD-3-Clause License

```
alloc-no-stdlib, alloc-stdlib, bindgen, brotli, brotli-decompressor, deunicode,
rust-stemmers, snap, subtle
```

#### BSD-2-Clause License

```
arrayref, Inflector, zerocopy (alternative)
```

#### ISC License

```
any_ascii, earcutr, libloading, ring, rustls, rustls-webpki, simple_asn1, untrusted
```

#### MPL-2.0 License (Mozilla Public License)

```
cssparser, cssparser-macros, dtoa-short, option-ext, selectors
```

#### Unicode-3.0 License

```
icu_collections, icu_locale_core, icu_normalizer, icu_normalizer_data, icu_properties,
icu_properties_data, icu_provider, litemap, potential_utf, tinystr, unicode-ident,
writeable, yoke, yoke-derive, zerofrom, zerofrom-derive, zerotrie, zerovec, zerovec-derive
```

#### Unlicense / Public Domain

```
aho-corasick, byteorder, ext-sort, fst, jiff, memchr, same-file, walkdir
```

#### CC0-1.0 (Public Domain)

```
blake3, constant_time_eq, dunce, tiny-keccak
```

#### 0BSD License

```
adler2
```

#### Zlib License

```
bytemuck, foldhash, tinyvec
```

#### CDLA-Permissive-2.0

```
webpki-roots
```

#### Special Licenses

| Crate | License | Notes |
|-------|---------|-------|
| surrealdb | BSL-1.1 | Business Source License, converts to Apache-2.0 after 4 years |
| surrealdb-core | BSL-1.1 | Same as surrealdb |
| dlopen2 | (not specified) | Dynamic library loading |
| dlopen2_derive | (not specified) | Proc macro for dlopen2 |
| encoding_rs | (Apache-2.0 OR MIT) AND BSD-3-Clause | Multiple licenses combined |
| ring | Apache-2.0 AND ISC | Combined license |
| dpi | Apache-2.0 AND MIT | Combined license |
| brotli | BSD-3-Clause AND MIT | Combined license |

---

## License Texts

### Apache License 2.0

The full text of the Apache License 2.0 is available in the LICENSE file at the root of this repository.

### MIT License

```
MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### BSD-3-Clause License

```
Redistribution and use in source and binary forms, with or without modification,
are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software without
   specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.
```

### ISC License

```
Permission to use, copy, modify, and/or distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT,
OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE,
DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS
ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

### MPL-2.0 (Mozilla Public License 2.0)

The full text is available at: https://www.mozilla.org/en-US/MPL/2.0/

Key points for Zileo Chat:
- You may use, modify, and distribute MPL-2.0 licensed code
- If you modify MPL-2.0 files, you must make those modifications available under MPL-2.0
- MPL-2.0 is file-level copyleft, not project-level (unlike GPL)

### Unicode License

```
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright (c) 1991-2024 Unicode, Inc. All rights reserved.

NOTICE TO USER: Carefully read the following legal agreement. BY DOWNLOADING,
INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR SOFTWARE, YOU
UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE TERMS AND CONDITIONS
OF THIS AGREEMENT.

Permission is hereby granted, free of charge, to any person obtaining a copy
of data files and any associated documentation (the "Data Files") or software
and any associated documentation (the "Software") to deal in the Data Files or
Software without restriction.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.
```

### BSL-1.1 (Business Source License)

SurrealDB is licensed under the Business Source License 1.1.

Key points:
- You may use the software for any purpose except as a database-as-a-service
- The license converts to Apache-2.0 after 4 years from release
- Full license text: https://github.com/surrealdb/surrealdb/blob/main/LICENSE

---

## Compliance Notes

1. **Attribution**: This file serves as the attribution notice required by various open-source licenses.

2. **Source Code**: For dependencies under licenses requiring source code availability, the source is available at the repository links listed above.

3. **Modifications**: Zileo Chat does not modify any third-party library source code directly. All dependencies are used as distributed.

4. **License Compatibility**: All licenses used are compatible with Apache-2.0 for the overall project.

---

## Updating This File

This file should be regenerated when dependencies are updated. Use the following commands:

```bash
# Rust dependencies with licenses
cd src-tauri && cargo tree --format "{p} {l}" --prefix none | sort -u

# Node.js dependencies
npm ls --all
```

---

*Last updated: 2025-12-11*
*Zileo Chat version: 0.9.0-beta*
