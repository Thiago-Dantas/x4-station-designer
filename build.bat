@echo off
rem Installs what x4sd needs to build (with winget) when it is missing, then builds target\release\x4sd.exe.
rem Details and the manual steps: BUILDING.md
setlocal EnableExtensions
cd /d "%~dp0"

where winget >nul 2>nul || (
  echo winget was not found. Install "App Installer" from the Microsoft Store, or install the tools by hand: see BUILDING.md.
  goto :fail
)
set "WG=--exact --accept-package-agreements --accept-source-agreements"

rem 1. MSVC C++ build tools and the Windows SDK. They come first: rustup's installer expects them.
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VCPATH="
if exist "%VSWHERE%" for /f "usebackq delims=" %%i in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VCPATH=%%i"
set "KITS="
set "SDKOK="
for /f "tokens=2,*" %%a in ('reg query "HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots" /v KitsRoot10 /reg:32 2^>nul') do set "KITS=%%b"
if defined KITS for /d %%d in ("%KITS%Lib\*") do if exist "%%d\um\x64\kernel32.lib" set "SDKOK=1"

if not defined VCPATH (
  echo Installing Visual Studio Build Tools with the C++ workload and the Windows SDK. This takes a while.
  winget install --id Microsoft.VisualStudio.BuildTools %WG% --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  if errorlevel 1 goto :fail
) else if not defined SDKOK (
  echo Installing the Windows SDK.
  winget install --id Microsoft.WindowsSDK.10.0.26100 %WG%
  if errorlevel 1 goto :fail
)

rem 2. Rust, stable MSVC toolchain
where cargo >nul 2>nul
if errorlevel 1 (
  echo Installing Rust.
  winget install --id Rustlang.Rustup %WG%
  if errorlevel 1 goto :fail
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
where cargo >nul 2>nul || (
  echo cargo is still not on PATH. Open a new terminal and run build.bat again.
  goto :fail
)
rustup default stable-msvc || goto :fail

rem 3. Node.js, for the builder UI
where npm >nul 2>nul
if errorlevel 1 (
  echo Installing Node.js LTS.
  winget install --id OpenJS.NodeJS.LTS %WG%
  if errorlevel 1 goto :fail
  set "PATH=%ProgramFiles%\nodejs;%PATH%"
)
where npm >nul 2>nul || (
  echo npm is still not on PATH. Open a new terminal and run build.bat again.
  goto :fail
)

rem 4. Build: the UI first, cargo embeds ui\dist into the exe
call npm --prefix ui ci || goto :fail
call npm --prefix ui run build || goto :fail
cargo build -p x4sd --release || goto :fail

echo.
echo Built %CD%\target\release\x4sd.exe
call :pause_if_double_clicked
exit /b 0

:fail
echo.
echo Build failed.
call :pause_if_double_clicked
exit /b 1

:pause_if_double_clicked
echo %cmdcmdline% | find /i "/c" >nul && pause
exit /b 0
