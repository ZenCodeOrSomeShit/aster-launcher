@echo off
setlocal
cd /d "%~dp0cosmetics-mod"
set "JAVA_HOME=%~dp0cosmetics-mod\.tooling\java\jdk-21.0.13+9"
set "GRADLE_USER_HOME=%~dp0cosmetics-mod\.tooling\gradle-cache"
call gradlew.bat runClient --console=plain
if errorlevel 1 pause
endlocal
