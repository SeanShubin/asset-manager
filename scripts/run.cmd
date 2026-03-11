@echo off
REM Launch asset-manager pointed at the seans-assets sibling directory.

set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%.."
for %%I in ("%REPO_ROOT%") do set "REPO_ROOT=%%~fI"
set "DATA_DIR=%REPO_ROOT%\..\seans-assets"
for %%I in ("%DATA_DIR%") do set "DATA_DIR=%%~fI"

if not exist "%DATA_DIR%" (
    mkdir "%DATA_DIR%"
    echo Created data directory: %DATA_DIR%
)

pushd "%REPO_ROOT%"
cargo run -- "%DATA_DIR%"
popd
