# Page Builder current-only cleanup failure

```text
[1m[92m    Updating[0m crates.io index
[1m[92m Downloading[0m crates ...
[1m[92m  Downloaded[0m anstyle v1.0.14
[1m[92m  Downloaded[0m aho-corasick v1.1.4
[1m[92m  Downloaded[0m anstyle-parse v1.0.0
[1m[92m  Downloaded[0m atomic-waker v1.1.2
[1m[92m  Downloaded[0m darling_core v0.20.11
[1m[92m  Downloaded[0m dotenvy v0.15.7
[1m[92m  Downloaded[0m attribute-derive v0.10.5
[1m[92m  Downloaded[0m erased v0.1.2
[1m[92m  Downloaded[0m allocator-api2 v0.2.21
[1m[92m  Downloaded[0m crypto-common v0.1.7
[1m[92m  Downloaded[0m deranged v0.5.8
[1m[92m  Downloaded[0m derive_more-impl v2.1.1
[1m[92m  Downloaded[0m convert_case v0.6.0
[1m[92m  Downloaded[0m fnv v1.0.7
[1m[92m  Downloaded[0m anstream v1.0.0
[1m[92m  Downloaded[0m cmake v0.1.58
[1m[92m  Downloaded[0m const_format v0.2.36
[1m[92m  Downloaded[0m convert_case_extras v0.2.0
[1m[92m  Downloaded[0m crossbeam-epoch v0.9.20
[1m[92m  Downloaded[0m ahash v0.7.8
[1m[92m  Downloaded[0m borsh v1.7.0
[1m[92m  Downloaded[0m aws-lc-rs v1.17.1
[1m[92m  Downloaded[0m form_urlencoded v1.2.2
[1m[92m  Downloaded[0m powerfmt v0.2.0
[1m[92m  Downloaded[0m foldhash v0.1.5
[1m[92m  Downloaded[0m aliasable v0.1.3
[1m[92m  Downloaded[0m opentelemetry v0.32.0
[1m[92m  Downloaded[0m autocfg v1.5.1
[1m[92m  Downloaded[0m const-oid v0.9.6
[1m[92m  Downloaded[0m sea-query-derive v0.4.3
[1m[92m  Downloaded[0m base64 v0.22.1
[1m[92m  Downloaded[0m rustix v0.38.44
[1m[92m  Downloaded[0m futures v0.3.32
[1m[92m  Downloaded[0m opentelemetry-http v0.32.0
[1m[92m  Downloaded[0m bitflags v2.13.0
[1m[92m  Downloaded[0m attribute-derive-macro v0.10.5
[1m[92m  Downloaded[0m block-buffer v0.10.4
[1m[92m  Downloaded[0m convert_case v0.11.0
[1m[92m  Downloaded[0m procfs v0.17.0
[1m[92m  Downloaded[0m or_poisoned v0.1.0
[1m[92m  Downloaded[0m ouroboros v0.18.5
[1m[92m  Downloaded[0m base64ct v1.8.3
[1m[92m  Downloaded[0m ordered-float v4.6.0
[1m[92m  Downloaded[0m concurrent-queue v2.5.0
[1m[92m  Downloaded[0m cpufeatures v0.3.0
[1m[92m  Downloaded[0m darling_macro v0.20.11
[1m[92m  Downloaded[0m any_spawner v0.3.0
[1m[92m  Downloaded[0m async-lock v3.4.2
[1m[92m  Downloaded[0m crypto-common v0.2.2
[1m[92m  Downloaded[0m event-listener-strategy v0.5.4
[1m[92m  Downloaded[0m flume v0.11.1
[1m[92m  Downloaded[0m hashlink v0.10.0
[1m[92m  Downloaded[0m konst_macro_rules v0.2.19
[1m[92m  Downloaded[0m opentelemetry_sdk v0.32.1
[1m[92m  Downloaded[0m async-stream v0.3.6
[1m[92m  Downloaded[0m cfg-if v1.0.4
[1m[92m  Downloaded[0m crossbeam-queue v0.3.13
[1m[92m  Downloaded[0m errno v0.3.14
[1m[92m  Downloaded[0m pem-rfc7468 v0.7.0
[1m[92m  Downloaded[0m opentelemetry-proto v0.32.0
[1m[92m  Downloaded[0m ouroboros_macro v0.18.5
[1m[92m  Downloaded[0m borsh-derive v1.7.0
[1m[92m  Downloaded[0m const_format_proc_macros v0.2.34
[1m[92m  Downloaded[0m drain_filter_polyfill v0.1.3
[1m[92m  Downloaded[0m rkyv_derive v0.7.46
[1m[92m  Downloaded[0m opentelemetry-otlp v0.32.0
[1m[92m  Downloaded[0m bytecheck_derive v0.6.12
[1m[92m  Downloaded[0m clap_derive v4.6.1
[1m[92m  Downloaded[0m collection_literals v1.0.3
[1m[92m  Downloaded[0m sea-orm-cli v1.1.20
[1m[92m  Downloaded[0m zeroize v1.9.0
[1m[92m  Downloaded[0m atoi v2.0.0
[1m[92m  Downloaded[0m clap v4.6.1
[1m[92m  Downloaded[0m const_str_slice_concat v0.1.0
[1m[92m  Downloaded[0m rustc-hash v2.1.3
[1m[92m  Downloaded[0m cfg_aliases v0.2.1
[1m[92m  Downloaded[0m codee v0.3.5
[1m[92m  Downloaded[0m either_of v0.1.9
[1m[92m  Downloaded[0m email_address v0.2.9
[1m[92m  Downloaded[0m rand_core v0.6.4
[1m[92m  Downloaded[0m rsa v0.9.10
[1m[92m  Downloaded[0m rust_decimal v1.42.1
[1m[92m  Downloaded[0m semver v1.0.28
[1m[92m  Downloaded[0m prost v0.14.4
[1m[92m  Downloaded[0m rustc_version v0.4.1
[1m[92m  Downloaded[0m regex-automata v0.4.15
[1m[92m  Downloaded[0m cpufeatures v0.2.17
[1m[92m  Downloaded[0m ptr_meta_derive v0.1.4
[1m[92m  Downloaded[0m rkyv v0.7.46
[1m[92m  Downloaded[0m reactive_stores_macro v0.4.3
[1m[92m  Downloaded[0m rustversion v1.0.23
[1m[92m  Downloaded[0m base16 v0.2.1
[1m[92m  Downloaded[0m bytecheck v0.6.12
[1m[92m  Downloaded[0m crc-catalog v2.5.0
[1m[92m  Downloaded[0m digest v0.10.7
[1m[92m  Downloaded[0m dunce v1.0.5
[1m[92m  Downloaded[0m chacha20 v0.10.1
[1m[92m  Downloaded[0m derive-where v1.6.1
[1m[92m  Downloaded[0m wasm_split_macros v0.2.2
[1m[92m  Downloaded[0m anstyle-query v1.1.5
[1m[92m  Downloaded[0m block-buffer v0.12.1
[1m[92m  Downloaded[0m clap_lex v1.1.0
[1m[92m  Downloaded[0m equivalent v1.0.2
[1m[92m  Downloaded[0m funty v2.0.0
[1m[92m  Downloaded[0m radium v0.7.0
[1m[92m  Downloaded[0m rand_pcg v0.10.2
[1m[92m  Downloaded[0m thread_local v1.1.10
[1m[92m  Downloaded[0m yoke-derive v0.8.2
[1m[92m  Downloaded[0m async-stream-impl v0.3.6
[1m[92m  Downloaded[0m generic-array v0.14.7
[1m[92m  Downloaded[0m sea-query-binder v0.7.0
[1m[92m  Downloaded[0m serde_urlencoded v0.7.1
[1m[92m  Downloaded[0m v_htmlescape v0.17.0
[1m[92m  Downloaded[0m displaydoc v0.2.6
[1m[92m  Downloaded[0m byteorder v1.5.0
[1m[92m  Downloaded[0m time-core v0.1.9
[1m[92m  Downloaded[0m wasm-bindgen-macro v0.2.126
[1m[92m  Downloaded[0m either v1.16.0
[1m[92m  Downloaded[0m heck v0.5.0
[1m[92m  Downloaded[0m scopeguard v1.2.0
[1m[92m  Downloaded[0m want v0.3.1
[1m[92m  Downloaded[0m digest v0.11.3
[1m[92m  Downloaded[0m find-msvc-tools v0.1.9
[1m[92m  Downloaded[0m percent-encoding v2.3.2
[1m[92m  Downloaded[0m pkcs8 v0.10.2
[1m[92m  Downloaded[0m potential_utf v0.1.5
[1m[92m  Downloaded[0m proc-macro2-diagnostics v0.10.1
[1m[92m  Downloaded[0m heck v0.4.1
[1m[92m  Downloaded[0m serde_spanned v1.1.1
[1m[92m  Downloaded[0m camino v1.2.4
[1m[92m  Downloaded[0m crossbeam-utils v0.8.22
[1m[92m  Downloaded[0m pin-project-internal v1.1.13
[1m[92m  Downloaded[0m pin-project-lite v0.2.17
[1m[92m  Downloaded[0m pkg-config v0.3.33
[1m[92m  Downloaded[0m reactive_stores v0.4.3
[1m[92m  Downloaded[0m rustls-platform-verifier v0.7.0
[1m[92m  Downloaded[0m const-oid v0.10.2
[1m[92m  Downloaded[0m darling v0.20.11
[1m[92m  Downloaded[0m wasm-bindgen-shared v0.2.126
[1m[92m  Downloaded[0m bytes v1.12.1
[1m[92m  Downloaded[0m sea-bae v0.2.1
[1m[92m  Downloaded[0m sea-schema-derive v0.3.0
[1m[92m  Downloaded[0m const-str v1.1.0
[1m[92m  Downloaded[0m fs_extra v1.3.0
[1m[92m  Downloaded[0m litemap v0.8.2
[1m[92m  Downloaded[0m wasm_split_helpers v0.2.3
[1m[92m  Downloaded[0m arrayvec v0.7.8
[1m[92m  Downloaded[0m hyper-rustls v0.27.9
[1m[92m  Downloaded[0m try-lock v0.2.5
[1m[92m  Downloaded[0m typed-builder-macro v0.23.2
[1m[92m  Downloaded[0m inventory v0.3.24
[1m[92m  Downloaded[0m proc-macro-utils v0.10.0
[1m[92m  Downloaded[0m quote-use-macros v0.8.4
[1m[92m  Downloaded[0m rend v0.4.2
[1m[92m  Downloaded[0m syn_derive v0.2.0
[1m[92m  Downloaded[0m config v0.15.25
[1m[92m  Downloaded[0m event-listener v5.4.1
[1m[92m  Downloaded[0m futures-channel v0.3.32
[1m[92m  Downloaded[0m hex v0.4.3
[1m[92m  Downloaded[0m inherent v1.0.13
[1m[92m  Downloaded[0m num-integer v0.1.46
[1m[92m  Downloaded[0m proc-macro-crate v3.5.0
[1m[92m  Downloaded[0m send_wrapper v0.6.0
[1m[92m  Downloaded[0m xxhash-rust v0.8.16
[1m[92m  Downloaded[0m async-trait v0.1.89
[1m[92m  Downloaded[0m lock_api v0.4.14
[1m[92m  Downloaded[0m pathdiff v0.2.3
[1m[92m  Downloaded[0m quote-use v0.8.4
[1m[92m  Downloaded[0m throw_error v0.3.1
[1m[92m  Downloaded[0m rand_chacha v0.3.1
[1m[92m  Downloaded[0m rustls-native-certs v0.8.4
[1m[92m  Downloaded[0m parking v2.2.1
[1m[92m  Downloaded[0m prost-types v0.14.4
[1m[92m  Downloaded[0m tinystr v0.8.3
[1m[92m  Downloaded[0m oco_ref v0.2.1
[1m[92m  Downloaded[0m pgvector v0.4.2
[1m[92m  Downloaded[0m prost-derive v0.14.4
[1m[92m  Downloaded[0m rand_chacha v0.9.0
[1m[92m  Downloaded[0m ipnet v2.12.0
[1m[92m  Downloaded[0m paste v1.0.15
[1m[92m  Downloaded[0m ppv-lite86 v0.2.21
[1m[92m  Downloaded[0m quinn-udp v0.5.15
[1m[92m  Downloaded[0m quote v1.0.46
[1m[92m  Downloaded[0m rand v0.8.7
[1m[92m  Downloaded[0m sqlx-macros-core v0.8.6
[1m[92m  Downloaded[0m crc v3.4.0
[1m[92m  Downloaded[0m futures-executor v0.3.32
[1m[92m  Downloaded[0m icu_properties v2.2.0
[1m[92m  Downloaded[0m md-5 v0.10.6
[1m[92m  Downloaded[0m parking_lot v0.12.5
[1m[92m  Downloaded[0m pkcs1 v0.7.5
[1m[92m  Downloaded[0m proc-macro-error-attr2 v2.0.0
[1m[92m  Downloaded[0m proc-macro-error2 v2.0.1
[1m[92m  Downloaded[0m ptr_meta v0.1.4
[1m[92m  Downloaded[0m rand_core v0.9.5
[1m[92m  Downloaded[0m rand_core v0.10.1
[1m[92m  Downloaded[0m reactive_graph v0.2.14
[1m[92m  Downloaded[0m rstml v0.12.1
[1m[92m  Downloaded[0m same-file v1.0.6
[1m[92m  Downloaded[0m sea-orm-macros v1.1.20
[1m[92m  Downloaded[0m seahash v4.1.0
[1m[92m  Downloaded[0m stringprep v0.1.5
[1m[92m  Downloaded[0m getrandom v0.2.17
[1m[92m  Downloaded[0m is_terminal_polyfill v1.70.2
[1m[92m  Downloaded[0m itoa v1.0.18
[1m[92m  Downloaded[0m leptos_dom v0.8.8
[1m[92m  Downloaded[0m leptos_server v0.8.7
[1m[92m  Downloaded[0m matchers v0.2.0
[1m[92m  Downloaded[0m once_cell v1.21.4
[1m[92m  Downloaded[0m parking_lot_core v0.9.12
[1m[92m  Downloaded[0m ryu v1.0.23
[1m[92m  Downloaded[0m subtle v2.6.1
[1m[92m  Downloaded[0m colorchoice v1.0.5
[1m[92m  Downloaded[0m futures-io v0.3.32
[1m[92m  Downloaded[0m html-escape v0.2.14
[1m[92m  Downloaded[0m http-body v1.0.1
[1m[92m  Downloaded[0m spin v0.9.8
[1m[92m  Downloaded[0m tinyvec v1.12.0
[1m[92m  Downloaded[0m tonic-prost v0.14.6
[1m[92m  Downloaded[0m typed-builder v0.23.2
[1m[92m  Downloaded[0m wasm-streams v0.5.0
[1m[92m  Downloaded[0m zerofrom v0.1.8
[1m[92m  Downloaded[0m cc v1.2.67
[1m[92m  Downloaded[0m derive_more v2.1.1
[1m[92m  Downloaded[0m gloo-net v0.6.0
[1m[92m  Downloaded[0m prettyplease v0.2.37
[1m[92m  Downloaded[0m der v0.7.10
[1m[92m  Downloaded[0m sha2 v0.10.9
[1m[92m  Downloaded[0m sync_wrapper v1.0.2
[1m[92m  Downloaded[0m tagptr v0.2.0
[1m[92m  Downloaded[0m crossbeam-channel v0.5.16
[1m[92m  Downloaded[0m interpolator v0.5.0
[1m[92m  Downloaded[0m gloo-utils v0.2.0
[1m[92m  Downloaded[0m guardian v1.3.0
[1m[92m  Downloaded[0m protobuf-support v3.7.2
[1m[92m  Downloaded[0m zerofrom-derive v0.1.7
[1m[92m  Downloaded[0m serde_yaml v0.9.34+deprecated
[1m[92m  Downloaded[0m synstructure v0.13.2
[1m[92m  Downloaded[0m utf8parse v0.2.2
[1m[92m  Downloaded[0m num-iter v0.1.46
[1m[92m  Downloaded[0m socket2 v0.6.4
[1m[92m  Downloaded[0m tower-service v0.3.3
[1m[92m  Downloaded[0m pin-project v1.1.13
[1m[92m  Downloaded[0m proc-macro2 v1.0.106
[1m[92m  Downloaded[0m quinn v0.11.11
[1m[92m  Downloaded[0m mac_address v1.1.8
[1m[92m  Downloaded[0m mime v0.3.17
[1m[92m  Downloaded[0m regex v1.13.0
[1m[92m  Downloaded[0m rustls-webpki v0.103.13
[1m[92m  Downloaded[0m simdutf8 v0.1.5
[1m[92m  Downloaded[0m sqlx-macros v0.8.6
[1m[92m  Downloaded[0m tracing-serde v0.2.0
[1m[92m  Downloaded[0m ulid v2.0.1
[1m[92m  Downloaded[0m zmij v1.0.22
[1m[92m  Downloaded[0m futures-task v0.3.32
[1m[92m  Downloaded[0m procfs-core v0.17.0
[1m[92m  Downloaded[0m prometheus v0.14.0
[1m[92m  Downloaded[0m sea-orm-migration v1.1.20
[1m[92m  Downloaded[0m sqlx-sqlite v0.8.6
[1m[92m  Downloaded[0m thiserror v1.0.69
[1m[92m  Downloaded[0m time-macros v0.2.31
[1m[92m  Downloaded[0m unicode-bidi v0.3.18
[1m[92m  Downloaded[0m utoipa v5.5.0
[1m[92m  Downloaded[0m futures-macro v0.3.32
[1m[92m  Downloaded[0m futures-sink v0.3.32
[1m[92m  Downloaded[0m icu_normalizer v2.2.0
[1m[92m  Downloaded[0m lazy_static v1.5.0
[1m[92m  Downloaded[0m memoffset v0.9.1
[1m[92m  Downloaded[0m num-traits v0.2.19
[1m[92m  Downloaded[0m portable-atomic v1.13.1
[1m[92m  Downloaded[0m rand v0.9.5
[1m[92m  Downloaded[0m rand v0.10.2
[1m[92m  Downloaded[0m reqwest v0.13.4
[1m[92m  Downloaded[0m serde_core v1.0.228
[1m[92m  Downloaded[0m server_fn v0.8.13
[1m[92m  Downloaded[0m shlex v2.0.1
[1m[92m  Downloaded[0m sqlx-mysql v0.8.6
[1m[92m  Downloaded[0m tokio-util v0.7.18
[1m[92m  Downloaded[0m v_escape-base v0.1.0
[1m[92m  Downloaded[0m bumpalo v3.20.3
[1m[92m  Downloaded[0m jobserver v0.1.35
[1m[92m  Downloaded[0m libm v0.2.16
[1m[92m  Downloaded[0m protobuf v3.7.2
[1m[92m  Downloaded[0m rustls v0.23.41
[1m[92m  Downloaded[0m sea-query v0.32.7
[1m[92m  Downloaded[0m stable_deref_trait v1.2.1
[1m[92m  Downloaded[0m strsim v0.11.1
[1m[92m  Downloaded[0m tap v1.0.1
[1m[92m  Downloaded[0m tracing-log v0.2.0
[1m[92m  Downloaded[0m tracing-subscriber v0.3.23
[1m[92m  Downloaded[0m whoami v1.6.1
[1m[92m  Downloaded[0m zerocopy v0.8.54
[1m[92m  Downloaded[0m futures-intrusive v0.5.0
[1m[92m  Downloaded[0m hybrid-array v0.4.13
[1m[92m  Downloaded[0m idna_adapter v1.2.2
[1m[92m  Downloaded[0m manyhow v0.11.4
[1m[92m  Downloaded[0m nix v0.29.0
[1m[92m  Downloaded[0m regex-syntax v0.8.11
[1m[92m  Downloaded[0m spki v0.7.3
[1m[92m  Downloaded[0m strum v0.26.3
[1m[92m  Downloaded[0m thiserror v2.0.18
[1m[92m  Downloaded[0m toml_datetime v1.1.1+spec-1.1.0
[1m[92m  Downloaded[0m unicode-properties v0.1.4
[1m[92m  Downloaded[0m zerovec v0.11.6
[1m[92m  Downloaded[0m leptos_macro v0.8.17
[1m[92m  Downloaded[0m serde_qs v0.15.0
[1m[92m  Downloaded[0m tracing-attributes v0.1.31
[1m[92m  Downloaded[0m walkdir v2.5.0
[1m[92m  Downloaded[0m webpki-roots v0.26.11
[1m[92m  Downloaded[0m icu_normalizer_data v2.2.0
[1m[92m  Downloaded[0m leptos_hot_reload v0.8.6
[1m[92m  Downloaded[0m libc v0.2.186
[1m[92m  Downloaded[0m sha1 v0.10.7
[1m[92m  Downloaded[0m tokio-rustls v0.26.4
[1m[92m  Downloaded[0m iana-time-zone v0.1.65
[1m[92m  Downloaded[0m mio v1.2.1
[1m[92m  Downloaded[0m server_fn_macro_default v0.8.5
[1m[92m  Downloaded[0m slab v0.4.12
[1m[92m  Downloaded[0m tracing-opentelemetry v0.33.0
[1m[92m  Downloaded[0m unicode-xid v0.2.6
[1m[92m  Downloaded[0m bigdecimal v0.4.10
[1m[92m  Downloaded[0m home v0.5.12
[1m[92m  Downloaded[0m static_assertions v1.1.0
[1m[92m  Downloaded[0m hydration_context v0.3.1
[1m[92m  Downloaded[0m serde_derive v1.0.228
[1m[92m  Downloaded[0m tokio-macros v2.7.0
[1m[92m  Downloaded[0m untrusted v0.9.0
[1m[92m  Downloaded[0m utf8_iter v1.0.4
[1m[92m  Downloaded[0m getrandom v0.3.4
[1m[92m  Downloaded[0m hmac v0.12.1
[1m[92m  Downloaded[0m hyper-timeout v0.5.2
[1m[92m  Downloaded[0m nu-ansi-term v0.50.3
[1m[92m  Downloaded[0m ident_case v1.0.1
[1m[92m  Downloaded[0m smallvec v1.15.2
[1m[92m  Downloaded[0m tokio-stream v0.1.18
[1m[92m  Downloaded[0m tracing-core v0.1.36
[1m[92m  Downloaded[0m zerovec-derive v0.11.3
[1m[92m  Downloaded[0m glob v0.3.3
[1m[92m  Downloaded[0m writeable v0.6.3
[1m[92m  Downloaded[0m num-conv v0.2.2
[1m[92m  Downloaded[0m unicode-ident v1.0.24
[1m[92m  Downloaded[0m wyz v0.5.1
[1m[92m  Downloaded[0m idna v1.1.0
[1m[92m  Downloaded[0m sha2 v0.11.0
[1m[92m  Downloaded[0m tonic v0.14.6
[1m[92m  Downloaded[0m tower-layer v0.3.3
[1m[92m  Downloaded[0m typenum v1.20.1
[1m[92m  Downloaded[0m wasm-bindgen v0.2.126
[1m[92m  Downloaded[0m yansi v1.0.1
[1m[92m  Downloaded[0m getrandom v0.4.3
[1m[92m  Downloaded[0m http v1.4.2
[1m[92m  Downloaded[0m hyper-util v0.1.20
[1m[92m  Downloaded[0m konst v0.2.20
[1m[92m  Downloaded[0m hkdf v0.12.4
[1m[92m  Downloaded[0m indexmap v2.14.0
[1m[92m  Downloaded[0m icu_locale_core v2.2.0
[1m[92m  Downloaded[0m bitvec v1.1.1
[1m[92m  Downloaded[0m clap_builder v4.6.0
[1m[92m  Downloaded[0m icu_collections v2.2.0
[1m[92m  Downloaded[0m memchr v2.8.3
[1m[92m  Downloaded[0m signal-hook-registry v1.4.8
[1m[92m  Downloaded[0m sqlx-core v0.8.6
[1m[92m  Downloaded[0m url v2.5.8
[1m[92m  Downloaded[0m uuid v1.23.5
[1m[92m  Downloaded[0m chrono v0.4.45
[1m[92m  Downloaded[0m hashbrown v0.12.3
[1m[92m  Downloaded[0m itertools v0.14.0
[1m[92m  Downloaded[0m lru-slab v0.1.2
[1m[92m  Downloaded[0m next_tuple v0.1.0
[1m[92m  Downloaded[0m num-bigint v0.4.8
[1m[92m  Downloaded[0m sqlx-postgres v0.8.6
[1m[92m  Downloaded[0m tachys v0.2.18
[1m[92m  Downloaded[0m thiserror-impl v1.0.69
[1m[92m  Downloaded[0m version_check v0.9.5
[1m[92m  Downloaded[0m hashbrown v0.15.5
[1m[92m  Downloaded[0m log v0.4.33
[1m[92m  Downloaded[0m server_fn_macro v0.8.10
[1m[92m  Downloaded[0m utoipa-gen v5.5.0
[1m[92m  Downloaded[0m wasm-bindgen-macro-support v0.2.126
[1m[92m  Downloaded[0m winnow v1.0.3
[1m[92m  Downloaded[0m futures-util v0.3.32
[1m[92m  Downloaded[0m hashbrown v0.17.1
[1m[92m  Downloaded[0m hyper v1.10.1
[1m[92m  Downloaded[0m icu_properties_data v2.2.0
[1m[92m  Downloaded[0m rustls-pki-types v1.15.0
[1m[92m  Downloaded[0m syn v1.0.109
[1m[92m  Downloaded[0m zerotrie v0.2.4
[1m[92m  Downloaded[0m syn v2.0.118
[1m[92m  Downloaded[0m thiserror-impl v2.0.18
[1m[92m  Downloaded[0m time v0.3.53
[1m[92m  Downloaded[0m toml_parser v1.1.2+spec-1.1.0
[1m[92m  Downloaded[0m tonic-types v0.14.6
[1m[92m  Downloaded[0m vcpkg v0.2.15
[1m[92m  Downloaded[0m webpki-roots v1.0.8
[1m[92m  Downloaded[0m httparse v1.10.1
[1m[92m  Downloaded[0m leptos_config v0.8.10
[1m[92m  Downloaded[0m openssl-probe v0.2.1
[1m[92m  Downloaded[0m signature v2.2.0
[1m[92m  Downloaded[0m serde v1.0.228
[1m[92m  Downloaded[0m web-sys v0.3.103
[1m[92m  Downloaded[0m toml v1.1.2+spec-1.1.0
[1m[92m  Downloaded[0m toml_edit v0.25.12+spec-1.1.0
[1m[92m  Downloaded[0m toml_writer v1.1.1+spec-1.1.0
[1m[92m  Downloaded[0m tracing v0.1.44
[1m[92m  Downloaded[0m unicode-normalization v0.1.25
[1m[92m  Downloaded[0m sharded-slab v0.1.7
[1m[92m  Downloaded[0m leptos v0.8.20
[1m[92m  Downloaded[0m slotmap v1.1.1
[1m[92m  Downloaded[0m unsafe-libyaml v0.2.11
[1m[92m  Downloaded[0m icu_provider v2.2.0
[1m[92m  Downloaded[0m tower v0.5.3
[1m[92m  Downloaded[0m encoding_rs v0.8.35
[1m[92m  Downloaded[0m serde_json v1.0.150
[1m[92m  Downloaded[0m unicode-segmentation v1.13.3
[1m[92m  Downloaded[0m h2 v0.4.15
[1m[92m  Downloaded[0m num-bigint-dig v0.8.6
[1m[92m  Downloaded[0m sqlx v0.8.6
[1m[92m  Downloaded[0m moka v0.12.15
[1m[92m  Downloaded[0m tower-http v0.6.11
[1m[92m  Downloaded[0m ring v0.17.14
[1m[92m  Downloaded[0m sea-orm v1.1.20
[1m[92m  Downloaded[0m tokio v1.52.3
[1m[92m  Downloaded[0m js-sys v0.3.103
[1m[92m  Downloaded[0m http-body-util v0.1.3
[1m[92m  Downloaded[0m linux-raw-sys v0.4.15
[1m[92m  Downloaded[0m aws-lc-sys v0.42.0
[1m[92m  Downloaded[0m sea-schema v0.16.2
[1m[92m  Downloaded[0m quinn-proto v0.11.16
[1m[92m  Downloaded[0m libsqlite3-sys v0.30.1
[1m[92m  Downloaded[0m futures-core v0.3.32
[1m[92m  Downloaded[0m anyhow v1.0.103
[1m[92m  Downloaded[0m wasm-bindgen-futures v0.4.76
[1m[92m  Downloaded[0m yoke v0.8.3
[1m[92m  Downloaded[0m async-once-cell v0.5.4
[1m[92m  Downloaded[0m tinyvec_macros v0.1.1
[1m[92m  Downloaded[0m manyhow-macros v0.11.4
[1m[92m   Compiling[0m proc-macro2 v1.0.106
[1m[92m   Compiling[0m quote v1.0.46
[1m[92m   Compiling[0m unicode-ident v1.0.24
[1m[92m   Compiling[0m serde_core v1.0.228
[1m[92m   Compiling[0m libc v0.2.186
[1m[92m    Checking[0m cfg-if v1.0.4
[1m[92m   Compiling[0m serde v1.0.228
[1m[92m    Checking[0m pin-project-lite v0.2.17
[1m[92m    Checking[0m memchr v2.8.3
[1m[92m    Checking[0m futures-core v0.3.32
[1m[92m    Checking[0m once_cell v1.21.4
[1m[92m    Checking[0m futures-sink v0.3.32
[1m[92m   Compiling[0m version_check v0.9.5
[1m[92m    Checking[0m slab v0.4.12
[1m[92m   Compiling[0m syn v2.0.118
[1m[92m    Checking[0m futures-channel v0.3.32
[1m[92m    Checking[0m itoa v1.0.18
[1m[92m    Checking[0m futures-io v0.3.32
[1m[92m    Checking[0m futures-task v0.3.32
[1m[92m   Compiling[0m thiserror v2.0.18
[1m[92m    Checking[0m bytes v1.12.1
[1m[92m    Checking[0m equivalent v1.0.2
[1m[92m    Checking[0m scopeguard v1.2.0
[1m[92m   Compiling[0m parking_lot_core v0.9.12
[1m[92m    Checking[0m lock_api v0.4.14
[1m[92m    Checking[0m hashbrown v0.17.1
[1m[92m    Checking[0m tracing-core v0.1.36
[1m[92m    Checking[0m stable_deref_trait v1.2.1
[1m[92m    Checking[0m log v0.4.33
[1m[92m   Compiling[0m shlex v2.0.1
[1m[92m    Checking[0m indexmap v2.14.0
[1m[92m   Compiling[0m jobserver v0.1.35
[1m[92m   Compiling[0m find-msvc-tools v0.1.9
[1m[92m   Compiling[0m cc v1.2.67
[1m[92m    Checking[0m errno v0.3.14
[1m[92m   Compiling[0m zmij v1.0.22
[1m[92m    Checking[0m percent-encoding v2.3.2
[1m[92m    Checking[0m signal-hook-registry v1.4.8
[1m[92m    Checking[0m socket2 v0.6.4
[1m[92m    Checking[0m mio v1.2.1
[1m[92m   Compiling[0m serde_json v1.0.150
[1m[92m   Compiling[0m pkg-config v0.3.33
[1m[92m   Compiling[0m generic-array v0.14.7
[1m[92m    Checking[0m writeable v0.6.3
[1m[92m    Checking[0m litemap v0.8.2
[1m[92m    Checking[0m utf8_iter v1.0.4
[1m[92m   Compiling[0m crossbeam-utils v0.8.22
[1m[92m   Compiling[0m icu_properties_data v2.2.0
[1m[92m   Compiling[0m synstructure v0.13.2
[1m[92m    Checking[0m base64 v0.22.1
[1m[92m   Compiling[0m icu_normalizer_data v2.2.0
[1m[92m   Compiling[0m cmake v0.1.58
[1m[92m   Compiling[0m dunce v1.0.5
[1m[92m   Compiling[0m fs_extra v1.3.0
[1m[92m   Compiling[0m aws-lc-sys v0.42.0
[1m[92m    Checking[0m zeroize v1.9.0
[1m[92m   Compiling[0m autocfg v1.5.1
[1m[92m    Checking[0m http v1.4.2
[1m[92m    Checking[0m getrandom v0.2.17
[1m[92m    Checking[0m typenum v1.20.1
[1m[92m   Compiling[0m serde_derive v1.0.228
[1m[92m   Compiling[0m futures-macro v0.3.32
[1m[92m   Compiling[0m thiserror-impl v2.0.18
[1m[92m   Compiling[0m zerofrom-derive v0.1.7
[1m[92m    Checking[0m futures-util v0.3.32
[1m[92m   Compiling[0m yoke-derive v0.8.2
[1m[92m    Checking[0m zerofrom v0.1.8
[1m[92m   Compiling[0m tracing-attributes v0.1.31
[1m[92m    Checking[0m yoke v0.8.3
[1m[92m   Compiling[0m zerovec-derive v0.11.3
[1m[92m   Compiling[0m tokio-macros v2.7.0
[1m[92m    Checking[0m tracing v0.1.44
[1m[92m   Compiling[0m displaydoc v0.2.6
[1m[92m    Checking[0m futures-executor v0.3.32
[1m[92m    Checking[0m smallvec v1.15.2
[1m[92m    Checking[0m zerovec v0.11.6
[1m[92m    Checking[0m parking_lot v0.12.5
[1m[92m    Checking[0m zerotrie v0.2.4
[1m[92m    Checking[0m tokio v1.52.3
[1m[92m    Checking[0m tinystr v0.8.3
[1m[92m    Checking[0m icu_locale_core v2.2.0
[1m[92m    Checking[0m potential_utf v0.1.5
[1m[92m    Checking[0m icu_collections v2.2.0
[1m[92m    Checking[0m subtle v2.6.1
[1m[92m   Compiling[0m semver v1.0.28
[1m[92m    Checking[0m icu_provider v2.2.0
[1m[92m   Compiling[0m rustc_version v0.4.1
[1m[92m    Checking[0m icu_properties v2.2.0
[1m[92m    Checking[0m icu_normalizer v2.2.0
[1m[92m    Checking[0m rustls-pki-types v1.15.0
[1m[92m   Compiling[0m ring v0.17.14
[1m[92m    Checking[0m idna_adapter v1.2.2
[1m[92m    Checking[0m form_urlencoded v1.2.2
[1m[92m   Compiling[0m aws-lc-rs v1.17.1
[1m[92m   Compiling[0m wasm-bindgen-shared v0.2.126
[1m[92m    Checking[0m idna v1.1.0
[1m[92m   Compiling[0m num-traits v0.2.19
[1m[92m    Checking[0m url v2.5.8
[1m[92m    Checking[0m concurrent-queue v2.5.0
[1m[92m    Checking[0m untrusted v0.9.0
[1m[92m   Compiling[0m rustversion v1.0.23
[1m[92m    Checking[0m parking v2.2.1
[1m[92m    Checking[0m event-listener v5.4.1
[1m[92m   Compiling[0m proc-macro-error-attr2 v2.0.0
[1m[92m   Compiling[0m rustls v0.23.41
[1m[92m   Compiling[0m zerocopy v0.8.54
[1m[92m   Compiling[0m proc-macro-error2 v2.0.1
[1m[92m    Checking[0m rand_core v0.10.1
[1m[92m   Compiling[0m getrandom v0.4.3
[1m[92m   Compiling[0m bumpalo v3.20.3
[1m[92m   Compiling[0m anyhow v1.0.103
[1m[92m   Compiling[0m wasm-bindgen-macro-support v0.2.126
[1m[92m   Compiling[0m wasm-bindgen v0.2.126
[1m[92m    Checking[0m tokio-stream v0.1.18
[1m[92m    Checking[0m tokio-util v0.7.18
[1m[92m   Compiling[0m async-trait v0.1.89
[1m[92m    Checking[0m bitflags v2.13.0
[1m[92m   Compiling[0m wasm-bindgen-macro v0.2.126
[1m[92m    Checking[0m ppv-lite86 v0.2.21
[1m[92m    Checking[0m either v1.16.0
[1m[92m    Checking[0m http-body v1.0.1
[1m[92m    Checking[0m fnv v1.0.7
[1m[92m   Compiling[0m httparse v1.10.1
[1m[92m    Checking[0m tower-service v0.3.3
[1m[92m   Compiling[0m unicode-xid v0.2.6
[1m[92m    Checking[0m try-lock v0.2.5
[1m[92m    Checking[0m atomic-waker v1.1.2
[1m[92m    Checking[0m want v0.3.1
[1m[92m    Checking[0m h2 v0.4.15
[1m[92m    Checking[0m block-buffer v0.10.4
[1m[92m    Checking[0m crypto-common v0.1.7
[1m[92m   Compiling[0m pin-project-internal v1.1.13
[1m[92m    Checking[0m js-sys v0.3.103
[1m[92m    Checking[0m ryu v1.0.23
[1m[92m   Compiling[0m itertools v0.14.0
[1m[92m    Checking[0m hyper v1.10.1
[1m[92m    Checking[0m pin-project v1.1.13
[1m[92m    Checking[0m digest v0.10.7
[1m[92m    Checking[0m uuid v1.23.5
[1m[92m    Checking[0m futures v0.3.32
[1m[92m    Checking[0m aho-corasick v1.1.4
[1m[92m    Checking[0m hex v0.4.3
[1m[92m   Compiling[0m libm v0.2.16
[1m[92m    Checking[0m iana-time-zone v0.1.65
[1m[92m    Checking[0m regex-syntax v0.8.11
[1m[92m   Compiling[0m thiserror v1.0.69
[1m[92m    Checking[0m ipnet v2.12.0
[1m[92m    Checking[0m hyper-util v0.1.20
[1m[92m    Checking[0m regex-automata v0.4.15
[1m[92m    Checking[0m chrono v0.4.45
[1m[92m    Checking[0m serde_urlencoded v0.7.1
[1m[92m    Checking[0m num-integer v0.1.46
[1m[92m   Compiling[0m thiserror-impl v1.0.69
[1m[92m   Compiling[0m bigdecimal v0.4.10
[1m[92m   Compiling[0m proc-macro2-diagnostics v0.10.1
[1m[92m    Checking[0m sync_wrapper v1.0.2
[1m[92m   Compiling[0m num-conv v0.2.2
[1m[92m   Compiling[0m unicode-segmentation v1.13.3
[1m[92m    Checking[0m tower-layer v0.3.3
[1m[92m   Compiling[0m rust_decimal v1.42.1
[1m[92m   Compiling[0m time-core v0.1.9
[1m[92m   Compiling[0m heck v0.4.1
[1m[92m   Compiling[0m time-macros v0.2.31
[1m[92m   Compiling[0m convert_case v0.11.0
[1m[92m    Checking[0m tower v0.5.3
[1m[92m    Checking[0m num-bigint v0.4.8
[1m[92m    Checking[0m deranged v0.5.8
[1m[92m   Compiling[0m yansi v1.0.1
[1m[92m    Checking[0m arrayvec v0.7.8
[1m[92m    Checking[0m allocator-api2 v0.2.21
[1m[92m    Checking[0m foldhash v0.1.5
[1m[92m   Compiling[0m getrandom v0.3.4
[1m[92m    Checking[0m powerfmt v0.2.0
[1m[92m    Checking[0m hashbrown v0.15.5
[1m[92m    Checking[0m time v0.3.53
[1m[92m    Checking[0m wasm-bindgen-futures v0.4.76
[1m[92m    Checking[0m http-body-util v0.1.3
[1m[92m    Checking[0m event-listener-strategy v0.5.4
[1m[92m    Checking[0m webpki-roots v1.0.8
[1m[92m    Checking[0m cpufeatures v0.2.17
[1m[92m    Checking[0m crc-catalog v2.5.0
[1m[92m   Compiling[0m vcpkg v0.2.15
[1m[92m    Checking[0m tinyvec_macros v0.1.1
[1m[92m   Compiling[0m ident_case v1.0.1
[1m[92m   Compiling[0m darling_core v0.20.11
[1m[92m   Compiling[0m libsqlite3-sys v0.30.1
[1m[92m    Checking[0m tinyvec v1.12.0
[1m[92m    Checking[0m crc v3.4.0
[1m[92m    Checking[0m sha2 v0.10.9
[1m[92m    Checking[0m webpki-roots v0.26.11
[1m[92m    Checking[0m async-lock v3.4.2
[1m[92m    Checking[0m hashlink v0.10.0
[1m[92m   Compiling[0m prost-derive v0.14.4
[1m[92m    Checking[0m futures-intrusive v0.5.0
[1m[92m    Checking[0m rand_core v0.6.4
[1m[92m    Checking[0m crossbeam-queue v0.3.13
[1m[92m    Checking[0m lazy_static v1.5.0
[1m[92m   Compiling[0m paste v1.0.15
[1m[92m    Checking[0m or_poisoned v0.1.0
[1m[92m   Compiling[0m proc-macro-utils v0.10.0
[1m[92m   Compiling[0m darling_macro v0.20.11
[1m[92m    Checking[0m rand_chacha v0.3.1
[1m[92m    Checking[0m prost v0.14.4
[1m[92m    Checking[0m rand_core v0.9.5
[1m[92m    Checking[0m unicode-normalization v0.1.25
[1m[92m    Checking[0m web-sys v0.3.103
[1m[92m    Checking[0m hyper-timeout v0.5.2
[1m[92m    Checking[0m hmac v0.12.1
[1m[92m   Compiling[0m const_format_proc_macros v0.2.34
[1m[92m    Checking[0m atoi v2.0.0
[1m[92m    Checking[0m opentelemetry v0.32.0
[1m[92m    Checking[0m spin v0.9.8
[1m[92m   Compiling[0m slotmap v1.1.1
[1m[92m    Checking[0m throw_error v0.3.1
[1m[92m    Checking[0m unicode-bidi v0.3.18
[1m[92m   Compiling[0m konst_macro_rules v0.2.19
[1m[92m    Checking[0m unicode-properties v0.1.4
[1m[92m    Checking[0m dotenvy v0.15.7
[1m[92m    Checking[0m openssl-probe v0.2.1
[1m[92m    Checking[0m rustls-native-certs v0.8.4
[1m[92m    Checking[0m stringprep v0.1.5
[1m[92m   Compiling[0m konst v0.2.20
[1m[92m    Checking[0m flume v0.11.1
[1m[92m    Checking[0m hkdf v0.12.4
[1m[92m    Checking[0m tonic v0.14.6
[1m[92m    Checking[0m rand_chacha v0.9.0
[1m[92m    Checking[0m rand v0.8.7
[1m[92m   Compiling[0m darling v0.20.11
[1m[92m    Checking[0m sharded-slab v0.1.7
[1m[92m    Checking[0m matchers v0.2.0
[1m[92m    Checking[0m md-5 v0.10.6
[1m[92m   Compiling[0m reactive_graph v0.2.14
[1m[92m   Compiling[0m server_fn_macro v0.8.10
[1m[92m    Checking[0m tracing-serde v0.2.0
[1m[92m   Compiling[0m derive-where v1.6.1
[1m[92m    Checking[0m tracing-log v0.2.0
[1m[92m    Checking[0m send_wrapper v0.6.0
[1m[92m    Checking[0m thread_local v1.1.10
[1m[92m    Checking[0m home v0.5.12
[1m[92m    Checking[0m cpufeatures v0.3.0
[1m[92m   Compiling[0m heck v0.5.0
[1m[92m   Compiling[0m camino v1.2.4
[1m[92m    Checking[0m utf8parse v0.2.2
[1m[92m    Checking[0m nu-ansi-term v0.50.3
[1m[92m    Checking[0m byteorder v1.5.0
[1m[92m    Checking[0m winnow v1.0.3
[1m[92m    Checking[0m whoami v1.6.1
[1m[92m   Compiling[0m rustix v0.38.44
[1m[92m    Checking[0m tracing-subscriber v0.3.23
[1m[92m    Checking[0m toml_parser v1.1.2+spec-1.1.0
[1m[92m    Checking[0m anstyle-parse v1.0.0
[1m[92m   Compiling[0m sea-query-derive v0.4.3
[1m[92m    Checking[0m rand v0.9.5
[1m[92m   Compiling[0m const_format v0.2.36
[1m[92m    Checking[0m hydration_context v0.3.1
[1m[92m    Checking[0m any_spawner v0.3.0
[1m[92m    Checking[0m tower-http v0.6.11
[1m[92m    Checking[0m regex v1.13.0
[1m[92m   Compiling[0m syn_derive v0.2.0
[1m[92m    Checking[0m ordered-float v4.6.0
[1m[92m    Checking[0m hybrid-array v0.4.13
[1m[92m   Compiling[0m inherent v1.0.13
[1m[92m    Checking[0m toml_datetime v1.1.1+spec-1.1.0
[1m[92m    Checking[0m serde_spanned v1.1.1
[1m[92m    Checking[0m encoding_rs v0.8.35
[1m[92m    Checking[0m guardian v1.3.0
[1m[92m    Checking[0m toml_writer v1.1.1+spec-1.1.0
[1m[92m    Checking[0m linux-raw-sys v0.4.15
[1m[92m    Checking[0m rustc-hash v2.1.3
[1m[92m    Checking[0m mime v0.3.17
[1m[92m    Checking[0m colorchoice v1.0.5
[1m[92m    Checking[0m is_terminal_polyfill v1.70.2
[1m[92m   Compiling[0m procfs v0.17.0
[1m[92m    Checking[0m anstyle-query v1.1.5
[1m[92m   Compiling[0m protobuf v3.7.2
[1m[92m    Checking[0m anstyle v1.0.14
[1m[92m   Compiling[0m xxhash-rust v0.8.16
[1m[92m    Checking[0m anstream v1.0.0
[1m[92m    Checking[0m toml v1.1.2+spec-1.1.0
[1m[92m    Checking[0m sea-query v0.32.7
[1m[92m    Checking[0m opentelemetry_sdk v0.32.1
[1m[92m    Checking[0m tonic-prost v0.14.6
[1m[92m    Checking[0m prost-types v0.14.4
[1m[92m   Compiling[0m manyhow-macros v0.11.4
[1m[92m   Compiling[0m quote-use-macros v0.8.4
[1m[92m    Checking[0m protobuf-support v3.7.2
[1m[92m   Compiling[0m reactive_stores_macro v0.4.3
[1m[92m    Checking[0m procfs-core v0.17.0
[1m[92m   Compiling[0m tachys v0.2.18
[1m[92m    Checking[0m clap_lex v1.1.0
[1m[92m   Compiling[0m prettyplease v0.2.37
[1m[92m    Checking[0m strsim v0.11.1
[1m[92m   Compiling[0m prometheus v0.14.0
[1m[92m    Checking[0m clap_builder v4.6.0
[1m[92m    Checking[0m reactive_stores v0.4.3
[1m[92m    Checking[0m opentelemetry-proto v0.32.0
[1m[92m   Compiling[0m quote-use v0.8.4
[1m[92m    Checking[0m tonic-types v0.14.6
[1m[92m   Compiling[0m manyhow v0.11.4
[1m[92m    Checking[0m block-buffer v0.12.1
[1m[92m    Checking[0m crypto-common v0.2.2
[1m[92m    Checking[0m gloo-utils v0.2.0
[1m[92m   Compiling[0m clap_derive v4.6.1
[1m[92m    Checking[0m chacha20 v0.10.1
[1m[92m    Checking[0m either_of v0.1.9
[1m[92m   Compiling[0m ouroboros_macro v0.18.5
[1m[92m   Compiling[0m sea-bae v0.2.1
[1m[92m   Compiling[0m derive_more-impl v2.1.1
[1m[92m   Compiling[0m server_fn v0.8.13
[1m[92m    Checking[0m oco_ref v0.2.1
[1m[92m   Compiling[0m async-stream-impl v0.3.6
[1m[92m    Checking[0m const_str_slice_concat v0.1.0
[1m[92m    Checking[0m const-oid v0.10.2
[1m[92m    Checking[0m erased v0.1.2
[1m[92m   Compiling[0m interpolator v0.5.0
[1m[92m   Compiling[0m same-file v1.0.6
[1m[92m   Compiling[0m portable-atomic v1.13.1
[1m[92m    Checking[0m next_tuple v0.1.0
[1m[92m   Compiling[0m collection_literals v1.0.3
[1m[92m    Checking[0m drain_filter_polyfill v0.1.3
[1m[92m   Compiling[0m crossbeam-epoch v0.9.20
[1m[92m    Checking[0m aliasable v0.1.3
[1m[92m    Checking[0m html-escape v0.2.14
[1m[92m    Checking[0m static_assertions v1.1.0
[1m[92m    Checking[0m ouroboros v0.18.5
[1m[92m    Checking[0m convert_case v0.6.0
[1m[92m   Compiling[0m attribute-derive-macro v0.10.5
[1m[92m    Checking[0m derive_more v2.1.1
[1m[92m   Compiling[0m walkdir v2.5.0
[1m[92m    Checking[0m async-stream v0.3.6
[1m[92m    Checking[0m digest v0.11.3
[1m[92m    Checking[0m clap v4.6.1
[1m[92m   Compiling[0m sea-orm-macros v1.1.20
[1m[92m    Checking[0m rand v0.10.2
[1m[92m    Checking[0m gloo-net v0.6.0
[1m[92m   Compiling[0m server_fn_macro_default v0.8.5
[1m[92m   Compiling[0m rstml v0.12.1
[1m[92m    Checking[0m wasm-streams v0.5.0
[1m[92m    Checking[0m tracing-opentelemetry v0.33.0
[1m[92m   Compiling[0m sea-schema-derive v0.3.0
[1m[92m   Compiling[0m leptos_macro v0.8.17
[1m[92m    Checking[0m serde_qs v0.15.0
[1m[92m   Compiling[0m typed-builder-macro v0.23.2
[1m[92m    Checking[0m glob v0.3.3
[1m[92m    Checking[0m inventory v0.3.24
[1m[92m   Compiling[0m base16 v0.2.1
[1m[92m    Checking[0m const-str v1.1.0
[1m[92m    Checking[0m pathdiff v0.2.3
[1m[92m    Checking[0m strum v0.26.3
[1m[92m    Checking[0m typed-builder v0.23.2
[1m[92m   Compiling[0m leptos_hot_reload v0.8.6
[1m[92m    Checking[0m config v0.15.25
[1m[92m   Compiling[0m wasm_split_macros v0.2.2
[1m[92m    Checking[0m sea-orm-cli v1.1.20
[1m[92m    Checking[0m ulid v2.0.1
[1m[92m    Checking[0m sha2 v0.11.0
[1m[92m   Compiling[0m attribute-derive v0.10.5
[1m[92m   Compiling[0m convert_case_extras v0.2.0
[1m[92m   Compiling[0m leptos v0.8.20
[1m[92m    Checking[0m fly v0.1.0 (/home/runner/work/RusTok/RusTok/crates/fly)
[1m[92m    Checking[0m codee v0.3.5
[1m[92m   Compiling[0m utoipa-gen v5.5.0
[1m[92m    Checking[0m crossbeam-channel v0.5.16
[1m[92m    Checking[0m tagptr v0.2.0
[1m[92m    Checking[0m v_escape-base v0.1.0
[1m[92m    Checking[0m async-once-cell v0.5.4
[1m[92m    Checking[0m unsafe-libyaml v0.2.11
[1m[92m    Checking[0m serde_yaml v0.9.34+deprecated
[1m[92m    Checking[0m wasm_split_helpers v0.2.3
[1m[92m    Checking[0m v_htmlescape v0.17.0
[1m[92m    Checking[0m moka v0.12.15
[1m[92m    Checking[0m utoipa v5.5.0
[1m[92m    Checking[0m leptos_server v0.8.7
[1m[92m    Checking[0m leptos_dom v0.8.8
[1m[92m    Checking[0m rustok-api v0.1.0 (/home/runner/work/RusTok/RusTok/crates/rustok-api)
[1m[92m    Checking[0m leptos_config v0.8.10
[1m[92m    Checking[0m email_address v0.2.9
[1m[92m    Checking[0m fly-ui v0.1.0 (/home/runner/work/RusTok/RusTok/crates/fly-ui)
[1m[92m    Checking[0m fly-leptos v0.1.0 (/home/runner/work/RusTok/RusTok/crates/fly-leptos)
[1m[92m    Checking[0m rustls-webpki v0.103.13
[1m[92m    Checking[0m sqlx-core v0.8.6
[1m[92m    Checking[0m tokio-rustls v0.26.4
[1m[92m    Checking[0m rustls-platform-verifier v0.7.0
[1m[92m    Checking[0m hyper-rustls v0.27.9
[1m[92m    Checking[0m reqwest v0.13.4
[1m[92m    Checking[0m opentelemetry-http v0.32.0
[1m[92m    Checking[0m opentelemetry-otlp v0.32.0
[1m[92m    Checking[0m sqlx-sqlite v0.8.6
[1m[92m    Checking[0m sqlx-postgres v0.8.6
[1m[92m    Checking[0m rustok-telemetry v0.1.0 (/home/runner/work/RusTok/RusTok/crates/rustok-telemetry)
[1m[92m    Checking[0m rustok-events v0.1.0 (/home/runner/work/RusTok/RusTok/crates/rustok-events)
[1m[92m    Checking[0m sqlx v0.8.6
[1m[92m    Checking[0m sea-query-binder v0.7.0
[1m[92m    Checking[0m sea-schema v0.16.2
[1m[92m    Checking[0m sea-orm v1.1.20
[1m[92m    Checking[0m sea-orm-migration v1.1.20
[1m[92m    Checking[0m rustok-core v0.1.0 (/home/runner/work/RusTok/RusTok/crates/rustok-core)
[1m[92m    Checking[0m rustok-page-builder v0.1.0 (/home/runner/work/RusTok/RusTok/crates/rustok-page-builder)
[1m[92m    Finished[0m `dev` profile [unoptimized + debuginfo] target(s) in 1m 36s
[1m[33mwarning[0m: the following packages contain code that will be rejected by a future version of Rust: proc-macro-error2 v2.0.1
[1m[92mnote[0m: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`
[1m[92m Downloading[0m crates ...
[1m[92m  Downloaded[0m fastrand v2.4.1
[1m[92m  Downloaded[0m tempfile v3.27.0
[1m[92m  Downloaded[0m quick-error v1.2.3
[1m[92m  Downloaded[0m wait-timeout v0.2.1
[1m[92m  Downloaded[0m bit-vec v0.8.0
[1m[92m  Downloaded[0m rusty-fork v0.3.1
[1m[92m  Downloaded[0m rustix v1.1.4
[1m[92m  Downloaded[0m rand_xorshift v0.4.0
[1m[92m  Downloaded[0m proptest v1.11.0
[1m[92m  Downloaded[0m bit-set v0.8.0
[1m[92m  Downloaded[0m unarray v0.1.4
[1m[92m  Downloaded[0m linux-raw-sys v0.12.1
[1m[92m   Compiling[0m cfg-if v1.0.4
[1m[92m   Compiling[0m proc-macro2 v1.0.106
[1m[92m   Compiling[0m libc v0.2.186
[1m[92m   Compiling[0m quote v1.0.46
[1m[92m   Compiling[0m rustix v1.1.4
[1m[92m   Compiling[0m getrandom v0.4.3
[1m[92m   Compiling[0m linux-raw-sys v0.12.1
[1m[92m   Compiling[0m bitflags v2.13.0
[1m[92m   Compiling[0m num-traits v0.2.19
[1m[92m   Compiling[0m serde_core v1.0.228
[1m[92m   Compiling[0m zerocopy v0.8.54
[1m[92m   Compiling[0m getrandom v0.3.4
[1m[92m   Compiling[0m rand_core v0.9.5
[1m[92m   Compiling[0m syn v2.0.118
[1m[92m   Compiling[0m once_cell v1.21.4
[1m[92m   Compiling[0m fastrand v2.4.1
[1m[92m   Compiling[0m wait-timeout v0.2.1
[1m[92m   Compiling[0m tempfile v3.27.0
[1m[92m   Compiling[0m fnv v1.0.7
[1m[92m   Compiling[0m bit-vec v0.8.0
[1m[92m   Compiling[0m quick-error v1.2.3
[1m[92m   Compiling[0m serde_json v1.0.150
[1m[92m   Compiling[0m serde_derive v1.0.228
[1m[92m   Compiling[0m thiserror-impl v2.0.18
[1m[92m   Compiling[0m ppv-lite86 v0.2.21
[1m[92m   Compiling[0m serde v1.0.228
[1m[92m   Compiling[0m rusty-fork v0.3.1
[1m[92m   Compiling[0m rand_chacha v0.9.0
[1m[92m   Compiling[0m bit-set v0.8.0
[1m[92m   Compiling[0m zmij v1.0.22
[1m[92m   Compiling[0m rand v0.9.5
[1m[92m   Compiling[0m rand_xorshift v0.4.0
[1m[92m   Compiling[0m unarray v0.1.4
[1m[92m   Compiling[0m itoa v1.0.18
[1m[92m   Compiling[0m memchr v2.8.3
[1m[92m   Compiling[0m regex-syntax v0.8.11
[1m[92m   Compiling[0m thiserror v2.0.18
[1m[92m   Compiling[0m proptest v1.11.0
[1m[92m   Compiling[0m fly v0.1.0 (/home/runner/work/RusTok/RusTok/crates/fly)
[1m[92m    Finished[0m `test` profile [unoptimized + debuginfo] target(s) in 17.37s
[1m[92m     Running[0m unittests src/lib.rs (target/debug/deps/fly-494faf045f0adba3)

running 177 tests
test action::tests::anonymous_action_diagnostics_use_the_shared_canonical_path ... ok
test action::tests::materialization_clears_stale_interaction_attributes ... ok
test action::tests::duplicate_forms_and_interaction_conflicts_are_rejected ... ok
test action::tests::actions_and_forms_materialize_to_native_and_custom_contracts ... ok
test action::tests::non_post_encoding_is_rejected ... ok
test action::tests::missing_form_and_unsafe_url_are_blocking_validation ... ok
test action::tests::network_paths_and_backslash_urls_are_blocking_validation ... ok
test asset::tests::component_patch_keeps_provider_reference ... ok
test asset::tests::catalog_normalizes_grapesjs_asset_shapes_without_losing_raw_data ... ok
test audit::tests::accessible_page_is_clean_of_errors ... ok
test audit::tests::audit_finds_accessibility_and_structure_issues ... ok
test binding::tests::commands_preserve_unknown_entries ... ok
test binding::tests::materialization_applies_fields_attributes_styles_and_fallbacks ... FAILED
test codec::tests::decode_hydrates_canonical_component_from_grapesjs_frame ... ok
test codec::tests::encode_refreshes_frame_component_from_canonical_tree ... ok
test command::patch::tests::typed_builders_resolve_set_remove_conflicts_deterministically ... ok
test command::patch::tests::typed_reserved_fields_set_and_clear_without_extension_leaks ... ok
test codec::tests::project_hash_matches_encoded_bytes_after_component_mutation ... ok
test command::tests::batch_is_atomic_and_creates_one_history_entry ... ok
test command::tests::binding_commands_participate_in_history ... ok
test command::tests::context_commands_participate_in_history ... ok
test command::tests::failed_batch_does_not_change_document_or_history ... ok
test command::tests::invalid_runtime_definitions_block_transaction ... ok
test command::tests::dynamic_commands_participate_in_history ... ok
test command::tests::snapshot_restore_is_hash_verified_and_participates_in_history ... ok
test component_visit::tests::immutable_and_mutable_walks_share_page_depth_and_path_contract ... ok
test command::tests::tampered_snapshot_does_not_change_document_or_history ... ok
test command::tests::style_patch_merges_and_can_remove_individual_properties ... ok
test context_contract::tests::contract_exposes_required_defaults_and_dependencies ... ok
test context_contract::tests::empty_definition_paths_are_rejected_even_though_root_resolution_exists ... ok
test context_contract::tests::strict_preflight_promotes_missing_and_type_mismatch ... ok
test context_dependency::tests::graph_orders_computed_dependencies ... ok
test context_dependency::tests::graph_rejects_input_computed_path_shadowing ... ok
test context_dependency::tests::graph_connects_computed_bindings_conditions_and_repeaters ... FAILED
test context_scenario::tests::duplicate_scenario_ids_are_rejected ... ok
test command::tests::translation_commands_participate_in_history ... ok
test context_json_schema::tests::exports_nested_json_schema_with_required_and_computed_metadata ... ok
test context_json_schema::tests::generates_example_and_materializes_computed_values ... ok
test context_scenario::tests::scenario_suite_preflights_each_host_owned_context ... ok
test context_schema::tests::applies_defaults_and_resolves_forward_computed_dependencies ... ok
test context_schema::tests::commands_preserve_opaque_entries ... ok
test context_schema::tests::validation_detects_dependency_cycles_and_default_type_errors ... ok
test dynamic::tests::commands_preserve_unknown_entries ... ok
test dynamic::tests::conditions_hide_components_without_mutating_source ... ok
test interaction_capability::tests::duplicate_capability_ids_are_rejected ... ok
test interaction_capability::tests::capability_input_kind_is_validated ... ok
test interaction_capability::tests::invalid_capability_identifier_has_domain_error ... ok
test dynamic::tests::repeaters_clone_interpolate_and_remap_style_rules ... FAILED
test interaction_capability::tests::permissive_policy_preserves_unknown_provider_compatibility ... ok
test interaction_capability::tests::strict_policy_rejects_unregistered_provider_forms ... ok
test interaction_capability_gate::tests::registered_capability_allows_publish ... ok
test interaction_capability_gate::tests::strict_capability_policy_blocks_publish_without_blocking_base_gate ... ok
test internal_link::tests::anonymous_component_diagnostics_use_the_shared_canonical_path ... ok
test interaction_route::tests::catalog_resolves_identical_locale_fallback_for_all_interactions ... ok
test internal_link::tests::fallback_href_is_used_when_target_page_has_no_slug ... ok
test internal_link::tests::internal_page_link_materializes_locale_specific_href ... ok
test internal_link::tests::missing_target_is_blocking_validation_and_clears_stale_href_at_runtime ... ok
test internal_link::tests::unencoded_query_and_backslash_fragment_are_rejected ... ok
test internal_link::tests::unsafe_fallback_and_network_base_path_are_rejected ... ok
test landing_readiness::evaluate::tests::component_form_conflicts_are_runtime_contracts ... ok
test landing_contract::tests::registry_manifest_contains_only_used_components_in_stable_order ... ok
test landing_readiness::tests::localized_metadata_counts_as_ready_content ... ok
test landing_contract::tests::static_artifact_is_not_emitted_for_unready_project ... ok
test landing_readiness::tests::localized_slug_diagnostics_are_classified_as_routes ... ok
test landing_readiness::tests::missing_landing_contracts_block_readiness ... ok
test landing_readiness::tests::structural_readiness_applies_schema_defaults_before_audit ... FAILED
test landing_contract::tests::static_artifact_is_deterministic_and_contains_complete_html ... ok
test landing_readiness::tests::required_locale_coverage_gaps_block_readiness_without_strict_locale_validation ... ok
test landing_readiness::tests::structural_readiness_does_not_require_runtime_instance_data ... ok
test locale_coverage::tests::coverage_discovers_optional_locales_without_policy ... FAILED
test locale_coverage::tests::coverage_reports_exact_translation_and_metadata_gaps ... ok
test locale_coverage::tests::invalid_policy_prevents_strict_readiness ... ok
test locale_policy::tests::invalid_runtime_locale_is_diagnosed_before_defaulting ... ok
test locale_policy::tests::legacy_locale_aliases_are_canonicalized ... ok
test landing_readiness::tests::structural_readiness_validates_binding_fallback_contracts ... FAILED
test locale_policy::tests::policy_commands_normalize_and_preserve_extensions ... ok
test landing_readiness::tests::unresolved_runtime_bound_action_is_a_publish_blocker ... FAILED
test locale_policy::tests::unsupported_runtime_locale_falls_back_to_project_default ... ok
test locale_policy::tests::runtime_policy_defaults_locale_and_merges_fallback_chain ... ok
test locale_policy::tests::required_locale_coverage_is_warning_until_enforcement_is_enabled ... ok
test localized_route::tests::unique_localized_slug_can_infer_locale ... ok
test localized_route::tests::duplicate_slug_for_same_locale_is_rejected_and_validated ... ok
test localized_route::tests::localized_slug_resolution_selects_page_and_render_locale ... ok
test page::tests::duplicate_page_ids_are_rejected ... ok
test page::tests::last_page_cannot_be_removed ... ok
test page::tests::page_commands_add_move_patch_and_remove ... ok
test page_metadata::tests::editing_plain_preview_replaces_only_the_selected_metadata_field ... ok
test page::tests::summaries_expose_page_identity_and_component_count ... ok
test page_metadata::tests::metadata_normalizes_empty_values_and_slug ... ok
test page_metadata::tests::localized_metadata_exposes_preview_and_round_trips_losslessly ... ok
test page_metadata::tests::open_graph_falls_back_to_standard_metadata ... ok
test page_metadata::tests::page_metadata_preserves_unknown_fields ... ok
test page_metadata_locale::tests::localized_metadata_is_selected_without_mutating_source_document ... ok
test page_metadata_locale::tests::metadata_uses_context_fallback_chain_and_reports_it ... ok
test page_metadata_locale::tests::unresolved_metadata_wrapper_is_preserved_losslessly ... ok
test placement::tests::location_tracks_parent_and_index ... ok
test landing_readiness::tests::warnings_only_block_when_policy_requires_it ... ok
test placement::tests::placement_allows_builtin_inside_container ... ok
test placement::tests::placement_rejects_leaf_parent ... ok
test render::tests::resolves_page_by_id_slug_and_index ... ok
test render::tests::storefront_renderer_sanitizes_html_and_emits_metadata ... ok
test placement::tests::placement_rejects_recursive_move ... ok
test render::tests::style_hooks_can_be_disabled_with_project_css ... ok
test render::tests::storefront_renderer_uses_style_hooks_without_editor_instrumentation ... ok
test registry::tests::form_and_list_children_are_explicitly_allowed ... ok
test runtime_gate::tests::current_context_gate_rejects_missing_required_data ... ok
test runtime_gate::tests::any_scenario_gate_allows_one_valid_scenario ... ok
test runtime_gate::tests::all_scenario_gate_requires_every_scenario_to_pass ... ok
test runtime_gate::tests::named_gate_rejects_missing_required_scenario ... ok
test registry::tests::landing_templates_have_visible_nested_content ... ok
test runtime_gate::tests::enabled_readiness_blocks_publish_only_when_landing_is_not_ready ... ok
test runtime_gate::tests::enabled_readiness_allows_a_stable_landing ... ok
test runtime_gate::tests::readiness_is_opt_in_and_does_not_block_existing_gate_policies ... ok
test runtime_locale::tests::context_and_value_fallback_chains_are_supported ... ok
test runtime_locale::tests::exact_locale_and_nested_values_are_materialized ... ok
test runtime_locale::tests::locale_tags_are_case_separator_and_subtag_sensitive ... ok
test runtime_locale::tests::regional_locale_falls_back_to_language ... ok
test runtime_locale::tests::unresolved_localized_value_is_preserved_losslessly ... ok
test runtime_pipeline::tests::invalid_context_contract_does_not_replace_localized_root_context ... ok
test runtime_gate::tests::readiness_audits_the_publish_context_after_runtime_bindings ... FAILED
test runtime_pipeline::tests::actions_and_forms_materialize_in_the_canonical_runtime_pipeline ... ok
test runtime_pipeline::tests::internal_page_links_materialize_after_bindings_and_repeaters ... ok
test runtime_pipeline::tests::locale_resolution_runs_before_computed_values_and_bindings ... FAILED
test runtime_pipeline::tests::localized_page_metadata_is_materialized_before_render_selection ... ok
test runtime_pipeline::tests::pipeline_exposes_effective_context_and_materialized_document ... FAILED
test runtime_pipeline::tests::project_translation_catalog_materializes_before_bindings ... FAILED
test runtime_pipeline::tests::project_locale_policy_defaults_before_translation_materialization ... FAILED
test runtime_pipeline::tests::runtime_binding_can_supply_action_before_native_materialization ... FAILED
test runtime_pipeline::tests::runtime_bound_navigation_conflict_is_validated_before_materialization ... FAILED
test runtime_scenario_release::tests::required_baseline_blocks_when_missing ... ok
test runtime_render::tests::runtime_renderer_applies_context_bindings_and_repeaters_in_order ... FAILED
test runtime_render::tests::runtime_renderer_emits_native_forms_and_locale_aware_actions ... ok
test runtime_scenario_release::tests::invalid_integrity_hash_blocks_release ... ok
test runtime_scenario_render::tests::matrix_captures_render_errors_per_case ... ok
test runtime_scenario_render::tests::matrix_carries_action_and_form_materialization_counters ... ok
test runtime_scenario_release::tests::stable_candidate_passes_strict_gate ... ok
test runtime_scenario_render::tests::matrix_groups_duplicate_outputs ... ok
test runtime_scenario_release::tests::visual_drift_passes_block_broken_but_not_strict_gate ... FAILED
test runtime_scenario_snapshot::tests::html_drift_requires_review ... FAILED
test runtime_scenario_snapshot::tests::identical_snapshots_are_stable ... ok
test runtime_scenario_render::tests::matrix_renders_distinct_scenario_outputs ... FAILED
test runtime_scenario_snapshot::tests::removing_a_scenario_is_breaking ... ok
test runtime_validation::tests::duplicate_localized_slugs_block_publish_validation ... ok
test runtime_validation::tests::missing_internal_page_link_target_blocks_publish_validation ... ok
test runtime_validation::tests::invalid_action_and_form_contracts_block_publish_validation ... ok
test safe_url::tests::accepts_supported_local_and_absolute_urls ... ok
test safe_url::tests::rejects_absolute_urls_without_authority_or_scheme_targets ... ok
test runtime_validation::tests::strict_project_locale_policy_promotes_missing_coverage_to_errors ... ok
test runtime_validation::tests::runtime_validation_combines_locale_translation_contract_dependency_binding_and_dynamic_diagnostics ... FAILED
test safe_url::tests::rejects_network_paths_backslashes_controls_and_unsafe_schemes ... ok
test snapshot::tests::anonymous_components_use_canonical_paths_in_diffs ... ok
test snapshot::tests::catalog_compares_snapshot_with_current ... ok
test snapshot::tests::missing_snapshot_is_explicit ... ok
test snapshot::tests::catalog_evicts_old_snapshots ... ok
test snapshot::tests::snapshots_restore_and_verify_hash ... ok
test style_rule::tests::component_media_rule_round_trips_through_grapesjs_shape ... ok
test snapshot::tests::structural_diff_tracks_component_changes ... ok
test style_rule::tests::upsert_preserves_unknown_rule_fields ... ok
test tests::clipboard_remaps_internal_references ... ok
test tests::grapesjs_round_trip_preserves_unknown_fields ... FAILED
test runtime_validation::tests::extending_canonical_report_does_not_duplicate_runtime_diagnostics ... ok
test tests::stable_id_assignment_avoids_existing_ids ... ok
test tests::revision_acknowledgement_detects_conflicts ... ok
test trait_model::tests::empty_optional_trait_removes_target ... ok
test trait_model::tests::registry_accepts_namespaced_provider_schemas ... ok
test trait_model::tests::registry_rejects_duplicate_or_un_namespaced_schemas ... ok
test trait_model::tests::select_and_url_traits_validate_input ... ok
test trait_model::tests::trait_patch_targets_attributes_and_fields ... ok
test translation::tests::catalog_materializes_into_binding_context ... ok
test translation::tests::commands_preserve_opaque_entries_and_support_removal ... ok
test translation::tests::locale_policy_commands_share_translation_transaction_surface ... FAILED
test translation::tests::validation_reports_duplicate_and_invalid_locale_definitions ... ok
test tests::validation_preserves_missing_provider_nodes ... ok
test tests::commands_and_history_are_transactional ... ok
test validation::tests::empty_project_is_invalid ... ok
test validation::tests::validates_pages_assets_orphan_rules_and_runtime_extensions ... ok
test tests::opaque_top_level_fields_round_trip ... ok

failures:

---- binding::tests::materialization_applies_fields_attributes_styles_and_fallbacks stdout ----
SOURCE DOCUMENT: ProjectDocument {
    project: GrapesProject {
        assets: [],
        styles: [],
        pages: [
            ProjectPage {
                id: None,
                component: Some(
                    Object(
                        ComponentObject {
                            id: Some(
                                "root",
                            ),
                            component_type: Some(
                                "wrapper",
                            ),
                            tag_name: None,
                            provider: None,
                            attributes: {},
                            style: None,
                            traits: [],
                            components: Nodes(
                                [
                                    Object(
                                        ComponentObject {
                                            id: Some(
                                                "title",
                                            ),
                                            component_type: Some(
                                                "heading",
                                            ),
                                            tag_name: None,
                                            provider: None,
                                            attributes: {
                                                "title": String("Static"),
                                            },
                                            style: Some(
                                                Object {
                                                    "color": String("black"),
                                                },
                                            ),
                                            traits: [],
                                            components: Nodes(
                                                [],
                                            ),
                                            extensions: {
                                                "content": String("Static"),
                                            },
                                        },
                                    ),
                                ],
                            ),
                            extensions: {},
                        },
                    ),
                ),
                frames: None,
                extensions: {},
            },
        ],
        extensions: {
            "flyRuntimeBindings": Array [
                Object {
                    "component_id": String("title"),
                    "id": String("title-content"),
                    "name": String("content"),
                    "path": String("page.title"),
                    "target": String("field"),
                    "transform": String("uppercase"),
                },
                Object {
                    "component_id": String("title"),
                    "fallback": String("Fallback"),
                    "id": String("title-attribute"),
                    "name": String("title"),
                    "path": String("page.tooltip"),
                    "target": String("attribute"),
                },
                Object {
                    "component_id": String("title"),
                    "id": String("title-color"),
                    "name": String("color"),
                    "path": String("theme.color"),
                    "target": String("style"),
                },
            ],
        },
    },
}

thread 'binding::tests::materialization_applies_fields_attributes_styles_and_fallbacks' (8541) panicked at crates/fly/src/binding.rs:553:9:
assertion `left == right` failed
  left: Some("Static")
 right: Some("HELLO")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- context_dependency::tests::graph_connects_computed_bindings_conditions_and_repeaters stdout ----

thread 'context_dependency::tests::graph_connects_computed_bindings_conditions_and_repeaters' (8561) panicked at crates/fly/src/context_dependency.rs:401:9:
assertion `left == right` failed
  left: Some(0)
 right: Some(1)

---- dynamic::tests::repeaters_clone_interpolate_and_remap_style_rules stdout ----

thread 'dynamic::tests::repeaters_clone_interpolate_and_remap_style_rules' (8573) panicked at crates/fly/src/dynamic.rs:961:9:
assertion `left == right` failed
  left: Some("{{item.title}} #{{index}}")
 right: Some("Two #1")

---- landing_readiness::tests::structural_readiness_applies_schema_defaults_before_audit stdout ----

thread 'landing_readiness::tests::structural_readiness_applies_schema_defaults_before_audit' (8596) panicked at crates/fly/src/landing_readiness/tests.rs:176:5:
[LandingReadinessIssue { category: Content, diagnostic: ValidationDiagnostic { severity: Info, code: "opaque_runtime_bindings", path: "project.runtime.bindings", message: "1 runtime binding entries are opaque and preserved" } }, LandingReadinessIssue { category: RuntimeContracts, diagnostic: ValidationDiagnostic { severity: Info, code: "runtime_context_unused_field", path: "hero.title", message: "declared runtime field `hero.title` is not consumed by the project" } }, LandingReadinessIssue { category: Content, diagnostic: ValidationDiagnostic { severity: Error, code: "landing_empty_heading", path: "project.pages[0].page.component.components[0]", message: "heading has no text or accessible label" } }]

---- locale_coverage::tests::coverage_discovers_optional_locales_without_policy stdout ----

thread 'locale_coverage::tests::coverage_discovers_optional_locales_without_policy' (8601) panicked at crates/fly/src/locale_coverage.rs:337:9:
assertion `left == right` failed
  left: ["de-de", "en"]
 right: ["en", "de-de"]

---- landing_readiness::tests::structural_readiness_validates_binding_fallback_contracts stdout ----

thread 'landing_readiness::tests::structural_readiness_validates_binding_fallback_contracts' (8598) panicked at crates/fly/src/landing_readiness/tests.rs:222:5:
assertion failed: !report.ready

---- landing_readiness::tests::unresolved_runtime_bound_action_is_a_publish_blocker stdout ----

thread 'landing_readiness::tests::unresolved_runtime_bound_action_is_a_publish_blocker' (8599) panicked at crates/fly/src/landing_readiness/tests.rs:274:5:
assertion failed: !report.ready

---- runtime_gate::tests::readiness_audits_the_publish_context_after_runtime_bindings stdout ----

thread 'runtime_gate::tests::readiness_audits_the_publish_context_after_runtime_bindings' (8641) panicked at crates/fly/src/runtime_gate.rs:495:9:
[ValidationDiagnostic { severity: Info, code: "opaque_runtime_bindings", path: "project.runtime.bindings", message: "1 runtime binding entries are opaque and preserved" }, ValidationDiagnostic { severity: Info, code: "runtime_context_unused_field", path: "page.title", message: "declared runtime field `page.title` is not consumed by the project" }, ValidationDiagnostic { severity: Error, code: "landing_empty_heading", path: "project.pages[0].page.component.components[0]", message: "heading has no text or accessible label" }, ValidationDiagnostic { severity: Error, code: "runtime_publish_readiness_rejected", path: "project.readiness", message: "landing readiness policy rejected publish with 1 blocking issue(s)" }]

---- runtime_pipeline::tests::locale_resolution_runs_before_computed_values_and_bindings stdout ----

thread 'runtime_pipeline::tests::locale_resolution_runs_before_computed_values_and_bindings' (8651) panicked at crates/fly/src/runtime_pipeline.rs:556:9:
assertion `left == right` failed
  left: Some("Static")
 right: Some("Привет мир")

---- runtime_pipeline::tests::pipeline_exposes_effective_context_and_materialized_document stdout ----

thread 'runtime_pipeline::tests::pipeline_exposes_effective_context_and_materialized_document' (8653) panicked at crates/fly/src/runtime_pipeline.rs:229:9:
assertion `left == right` failed
  left: 0
 right: 1

---- runtime_pipeline::tests::project_translation_catalog_materializes_before_bindings stdout ----

thread 'runtime_pipeline::tests::project_translation_catalog_materializes_before_bindings' (8655) panicked at crates/fly/src/runtime_pipeline.rs:607:9:
assertion `left == right` failed
  left: 0
 right: 1

---- runtime_pipeline::tests::project_locale_policy_defaults_before_translation_materialization stdout ----

thread 'runtime_pipeline::tests::project_locale_policy_defaults_before_translation_materialization' (8654) panicked at crates/fly/src/runtime_pipeline.rs:285:9:
assertion `left == right` failed
  left: Some("Static")
 right: Some("Добро пожаловать")

---- runtime_pipeline::tests::runtime_binding_can_supply_action_before_native_materialization stdout ----

thread 'runtime_pipeline::tests::runtime_binding_can_supply_action_before_native_materialization' (8656) panicked at crates/fly/src/runtime_pipeline.rs:452:9:
assertion `left == right` failed
  left: 0
 right: 1

---- runtime_pipeline::tests::runtime_bound_navigation_conflict_is_validated_before_materialization stdout ----

thread 'runtime_pipeline::tests::runtime_bound_navigation_conflict_is_validated_before_materialization' (8657) panicked at crates/fly/src/runtime_pipeline.rs:500:9:
assertion failed: materialized.diagnostics.iter().any(|diagnostic|
        {
            diagnostic.code == "component_navigation_contract_conflict" &&
                diagnostic.severity == ValidationSeverity::Error
        })

---- runtime_render::tests::runtime_renderer_applies_context_bindings_and_repeaters_in_order stdout ----

thread 'runtime_render::tests::runtime_renderer_applies_context_bindings_and_repeaters_in_order' (8658) panicked at crates/fly/src/runtime_render.rs:169:9:
assertion `left == right` failed
  left: 0
 right: 1

---- runtime_scenario_release::tests::visual_drift_passes_block_broken_but_not_strict_gate stdout ----

thread 'runtime_scenario_release::tests::visual_drift_passes_block_broken_but_not_strict_gate' (8663) panicked at crates/fly/src/runtime_scenario_release.rs:431:9:
assertion `left == right` failed
  left: Stable
 right: RequiresReview

---- runtime_scenario_snapshot::tests::html_drift_requires_review stdout ----

thread 'runtime_scenario_snapshot::tests::html_drift_requires_review' (8668) panicked at crates/fly/src/runtime_scenario_snapshot.rs:394:9:
assertion `left == right` failed
  left: Stable
 right: RequiresReview

---- runtime_scenario_render::tests::matrix_renders_distinct_scenario_outputs stdout ----

thread 'runtime_scenario_render::tests::matrix_renders_distinct_scenario_outputs' (8667) panicked at crates/fly/src/runtime_scenario_render.rs:250:9:
assertion `left == right` failed
  left: 1
 right: 2

---- runtime_validation::tests::runtime_validation_combines_locale_translation_contract_dependency_binding_and_dynamic_diagnostics stdout ----

thread 'runtime_validation::tests::runtime_validation_combines_locale_translation_contract_dependency_binding_and_dynamic_diagnostics' (8675) panicked at crates/fly/src/runtime_validation.rs:127:9:
assertion failed: diagnostics.iter().any(|diagnostic|
        diagnostic.code == "runtime_binding_target_missing")

---- tests::grapesjs_round_trip_preserves_unknown_fields stdout ----

thread 'tests::grapesjs_round_trip_preserves_unknown_fields' (8690) panicked at crates/fly/src/tests.rs:18:5:
assertion `left == right` failed
  left: Object {"assets": Array [], "futureTopLevelField": Object {"enabled": Bool(true), "nested": Object {"value": Number(42)}}, "pages": Array [Object {"component": Object {"components": Array [Object {"attributes": Object {"data-query": String("latest")}, "components": Array [], "futureField": Object {"nested": Array [Number(1), Number(2), Number(3)]}, "id": String("widget-1"), "pluginMetadata": Object {"opaque": Bool(true), "version": String("future")}, "provider": String("rustok.forum"), "type": String("rustok.forum.latest_topics")}], "id": String("root"), "type": String("wrapper")}, "frames": Array [Object {"component": Object {"components": Array [Object {"attributes": Object {"data-query": String("latest")}, "components": Array [], "futureField": Object {"nested": Array [Number(1), Number(2), Number(3)]}, "id": String("widget-1"), "pluginMetadata": Object {"opaque": Bool(true), "version": String("future")}, "provider": String("rustok.forum"), "type": String("rustok.forum.latest_topics")}], "id": String("root"), "type": String("wrapper")}}], "futurePageField": Object {"keep": String("all")}, "id": String("provider-page")}], "styles": Array []}
 right: Object {"assets": Array [], "futureTopLevelField": Object {"enabled": Bool(true), "nested": Object {"value": Number(42)}}, "pages": Array [Object {"component": Object {"components": Array [Object {"attributes": Object {"data-query": String("latest")}, "components": Array [], "futureField": Object {"nested": Array [Number(1), Number(2), Number(3)]}, "id": String("widget-1"), "pluginMetadata": Object {"opaque": Bool(true), "version": String("future")}, "provider": String("rustok.forum"), "type": String("rustok.forum.latest_topics")}], "id": String("root"), "type": String("wrapper")}, "futurePageField": Object {"keep": String("all")}, "id": String("provider-page")}], "styles": Array []}

---- translation::tests::locale_policy_commands_share_translation_transaction_surface stdout ----

thread 'translation::tests::locale_policy_commands_share_translation_transaction_surface' (8702) panicked at crates/fly/src/translation.rs:448:10:
set locale policy: Decode("required locale `ru` is not present in supported_locales")


failures:
    binding::tests::materialization_applies_fields_attributes_styles_and_fallbacks
    context_dependency::tests::graph_connects_computed_bindings_conditions_and_repeaters
    dynamic::tests::repeaters_clone_interpolate_and_remap_style_rules
    landing_readiness::tests::structural_readiness_applies_schema_defaults_before_audit
    landing_readiness::tests::structural_readiness_validates_binding_fallback_contracts
    landing_readiness::tests::unresolved_runtime_bound_action_is_a_publish_blocker
    locale_coverage::tests::coverage_discovers_optional_locales_without_policy
    runtime_gate::tests::readiness_audits_the_publish_context_after_runtime_bindings
    runtime_pipeline::tests::locale_resolution_runs_before_computed_values_and_bindings
    runtime_pipeline::tests::pipeline_exposes_effective_context_and_materialized_document
    runtime_pipeline::tests::project_locale_policy_defaults_before_translation_materialization
    runtime_pipeline::tests::project_translation_catalog_materializes_before_bindings
    runtime_pipeline::tests::runtime_binding_can_supply_action_before_native_materialization
    runtime_pipeline::tests::runtime_bound_navigation_conflict_is_validated_before_materialization
    runtime_render::tests::runtime_renderer_applies_context_bindings_and_repeaters_in_order
    runtime_scenario_release::tests::visual_drift_passes_block_broken_but_not_strict_gate
    runtime_scenario_render::tests::matrix_renders_distinct_scenario_outputs
    runtime_scenario_snapshot::tests::html_drift_requires_review
    runtime_validation::tests::runtime_validation_combines_locale_translation_contract_dependency_binding_and_dynamic_diagnostics
    tests::grapesjs_round_trip_preserves_unknown_fields
    translation::tests::locale_policy_commands_share_translation_transaction_surface

test result: FAILED. 156 passed; 21 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

[1m[91merror[0m: test failed, to rerun pass `-p fly --lib`
```
