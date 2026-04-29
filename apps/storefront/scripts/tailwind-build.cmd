@echo off
setlocal

cd /d "%~dp0.."

set "OUT_DIR=%TRUNK_STAGING_DIR%"
if "%OUT_DIR%"=="" set "OUT_DIR=dist"

if not exist "node_modules\.bin\tailwindcss.cmd" (
  echo Missing apps\storefront node_modules. Run npm.cmd install in apps\storefront first.
  exit /b 1
)

call "node_modules\.bin\tailwindcss.cmd" -i assets\input.css -o "%OUT_DIR%\app.css" --minify
type nul > "%OUT_DIR%\.gitkeep"
