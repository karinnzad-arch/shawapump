# Bismillah Control Panel

⚠️ **No pude compilar ni probar esto en mi entorno** — solo tengo Node
instalado ahí, no Rust/Cargo. Escribí el código siguiendo el patrón
oficial de Tauri v2, pero **la compilación real la tenés que hacer vos**
en una máquina con Rust instalado. Te dejo los pasos exactos abajo,
incluyendo qué hacer si el compilador se queja de algo.

## Qué es esto

Una "carcaza" (app de escritorio) para tu bot:
- Lee y escribe tu `.env` real, preservando comentarios y orden
- Prende / apaga / reinicia el proceso del bot como hijo del suyo
- **Cero llamadas de red** — solo filesystem y procesos locales. Podés
  confirmarlo vos mismo buscando en `src-tauri/src/main.rs`: no hay
  ningún `reqwest`, `http`, ni nada que hable con internet.
- Genera un instalador nativo de Windows (`.msi`) al final

## Paso 1 — Instalar las herramientas necesarias (una sola vez)

En una PC con Windows (para que el instalador salga nativo):

1. **Rust**: https://rustup.rs — descargá y corré el instalador
2. **Node.js** (v18+): https://nodejs.org
3. **Requisitos de Tauri en Windows**: Microsoft C++ Build Tools —
   seguí exactamente esta guía oficial antes de continuar:
   https://tauri.app/start/prerequisites/

## Paso 2 — Preparar el proyecto

Copiá esta carpeta `bismillah-panel/` a tu PC Windows, abrí una
terminal (PowerShell) adentro, y corré:

```powershell
npm install
```

## Paso 3 — Probar en modo desarrollo (antes de generar el instalador)

```powershell
npm run tauri dev
```

Esto abre la app en una ventana, sin instalar nada todavía — es la
forma más rápida de ver si compila y de iterar sobre bugs. Si tira
error acá, revisá la sección "Problemas esperados" más abajo.

## Paso 4 — Generar el instalador de Windows

```powershell
npm run tauri build
```

Al terminar (puede tardar varios minutos la primera vez), el instalador
queda en:
```
src-tauri/target/release/bundle/msi/Bismillah Control Panel_1.0.0_x64_en-US.msi
```

Ese `.msi` es el que le pasás a quien quiera instalarlo — doble click,
"Siguiente, Siguiente, Instalar", como cualquier programa de Windows.

## Paso 5 — Primer uso

Al abrir la app por primera vez, te va a pedir:
1. Ubicación de tu `bismillah_bot.exe` compilado
2. Ubicación de tu archivo `.env`

Esto se guarda localmente (en `%APPDATA%/com.bismillah.controlpanel/`)
para que no te lo vuelva a preguntar la próxima vez.

## Problemas esperados al compilar (y cómo resolverlos)

**Si tira error de `tauri-plugin-dialog` o versión de `tauri`:**
La API de Tauri cambia entre versiones menores. Mirá el error exacto,
y comparalo contra la documentación de la versión que te instaló
`cargo`: https://docs.rs/tauri/latest/tauri/ y
https://docs.rs/tauri-plugin-dialog/latest/tauri_plugin_dialog/

**Si el instalador sin firmar dispara alerta de Windows SmartScreen:**
Es esperado — no significa que esté mal armado, solo que Windows no
reconoce quién firmó el `.exe`. Quien lo instale tiene que clickear
"Más información" → "Ejecutar de todas formas". Para sacarte esa
alerta de encima necesitarías un certificado de firma de código
(~$100-300/año), no es gratis.

**Si `npm run tauri dev` no encuentra `cargo`:**
Cerrá y volvé a abrir la terminal después de instalar Rust (rustup
necesita que reinicies la terminal para actualizar el PATH).

## Sobre `WALLET_PRIVATE_KEY`

Está marcado en el código como campo de contraseña (se ve como
`••••••`), y el warning box en la pestaña "Wallet" te lo recuerda en
la interfaz. Pero tené en cuenta: **el valor sigue viviendo en texto
plano dentro del archivo `.env`** — este panel no lo cifra, solo lo
oculta visualmente en pantalla. La seguridad real depende de que ese
`.env` esté protegido en tu disco (permisos de archivo, disco cifrado,
etc.), no de esta app.

## Qué falta para que quede "producción"

Esto es la versión simple que pediste — reinicia el bot siempre al
guardar. Cosas que valdría la pena sumar más adelante, si te sirve:
- Reemplazar los íconos placeholder (círculo verde genérico) por tu
  logo real: `npx @tauri-apps/cli icon ruta/a/tu/logo.png` genera
  todos los tamaños automáticamente
- Validación de tipos en los campos (que `MAX_POSITIONS` no acepte
  texto, por ejemplo) — ahora mismo todo se trata como texto plano
- Un log/consola dentro de la misma app mostrando el stdout del bot
  en vivo, en vez de tener que abrir una terminal aparte
