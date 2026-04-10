@echo off
cargo build --release
if %errorlevel% neq 0 exit /b %errorlevel%
rd /s /q dist 2>nul
mkdir dist
copy target\release\asteroids_3d.exe dist\
xcopy target\release\res\ dist\ /E /I /Y
echo Build and packaging complete. The dist folder contains the shareable executable and assets.