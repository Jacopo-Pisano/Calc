# ISO → XEX per Xbox

App Android che converte un'immagine `.iso` di un gioco **Xbox 360** (o Xbox
originale) in una cartella con `default.xex` e tutti i file del gioco, pronta
da avviare dalla console.

## Cosa fa davvero la "conversione"

Un'ISO Xbox 360 non si trasforma in un singolo `.xex`: contiene un filesystem
(XDVDFS) con dentro `default.xex`, cioè l'eseguibile, e tutti i dati del gioco.
Le dashboard homebrew (Aurora, FreeStyle Dash, XeXMenu) avviano il gioco da una
cartella che contiene `default.xex`. L'app quindi:

1. riconosce il tipo di disco (XGD1, XGD2, XGD3 o ISO già ricostruita);
2. legge l'header di `default.xex` / `default.xbe` e mostra Title ID, Media ID,
   versione e numero del disco;
3. estrae tutti i file in una cartella `NomeISO [TitleID]` nella posizione
   scelta (memoria interna, SD, chiavetta USB OTG).

> Gli XEX estratti si avviano solo su console modificate (RGH/JTAG). Usa solo
> copie di backup dei giochi che possiedi.

## Struttura

| Cartella | Contenuto |
|---|---|
| `core/` | Libreria **Rust**: parser XDVDFS, lettura header XEX2/XBE, binding JNI, CLI desktop `xiso2xex` |
| `android/` | App **Kotlin** + Jetpack Compose: UI, selezione file (Storage Access Framework), progresso |
| `.github/workflows/android.yml` | CI: test Rust + compilazione dell'APK |

Tutta la logica di lettura dell'ISO e la copia dei dati è in Rust; Kotlin si
occupa solo dell'interfaccia e dei permessi di Android, perché un'app Android
interamente in Rust non può usare in modo pratico i selettori di file del
sistema.

## Scaricare l'APK

Ogni push esegue il workflow **Build APK** su GitHub Actions: l'APK si trova
negli *Artifacts* dell'esecuzione (`xiso2xex-apk`). È firmato con la chiave di
debug, quindi si installa direttamente (abilita "installa app sconosciute").

## Compilare in locale

Requisiti: Android SDK + NDK, Rust, `cargo install cargo-ndk`.

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cd android
./gradlew assembleRelease   # compila anche il core Rust tramite cargo-ndk
```

CLI desktop (Linux/macOS/Windows):

```sh
cd core
cargo run --release --bin xiso2xex -- gioco.iso cartella_output
cargo test
```
