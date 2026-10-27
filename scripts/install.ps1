# Install, update or remove Nuvora on Windows, with Docker Desktop.
#
#   irm https://getnuvora.online/install.ps1 | iex
#
# That address redirects here. The installer finds Docker Desktop (and helps
# install or start it), asks where your movies and shows are, and runs the
# published image (ghcr.io/iucc123/nuvora) from ~\nuvora. Running it again
# updates Nuvora and keeps your answers. To pass options, run it like this:
#
#   & ([scriptblock]::Create((irm https://getnuvora.online/install.ps1))) -Media D:\Media
#
# It never calls exit: pasted into a window, that would close the window.
param(
  [string]$Media = "",
  [int]$Port = 0,
  [string]$Dir = "",
  [string]$Version = "",
  [switch]$Uninstall,
  [switch]$Yes
)

& {
  $ImageRepo = "ghcr.io/iucc123/nuvora"
  $InstallCmd = "irm https://getnuvora.online/install.ps1 | iex"
  $DesktopUrl = "https://www.docker.com/products/docker-desktop/"
  $DefaultPort = 8640
  $Stop = "nuvora-installer-stop"
  $IsWin = $env:OS -eq "Windows_NT"

  # Windows Terminal draws these; the old console window may not.
  $fancy = [bool]$env:WT_SESSION -or -not $IsWin
  if ($fancy) {
    $Tick = [string][char]0x2713; $Cross = [string][char]0x2717; $To = [string][char]0x2192
    $Frames = 0x280B, 0x2819, 0x2839, 0x2838, 0x283C, 0x2834, 0x2826, 0x2827, 0x2807, 0x280F | ForEach-Object { [string][char]$_ }
  } else {
    $Tick = "+"; $Cross = "x"; $To = "->"; $Frames = "-", "\", "|", "/"
  }
  $Interactive = -not $Yes -and [Environment]::UserInteractive -and -not [Console]::IsInputRedirected
  $Tmp = Join-Path ([IO.Path]::GetTempPath()) ("nuvora-install-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
  $null = New-Item -ItemType Directory -Path $Tmp
  # Shared state; under iex, $script: variables would land in the caller's session.
  $Ctx = @{ Log = ""; Docker = "docker"; MediaArg = $Media; MediaCount = 0; Crashed = $false; DirGiven = [bool]$Dir }

  function Say($t) { Write-Host "  $t" }
  function Ok($t) { Write-Host "  $Tick " -ForegroundColor Green -NoNewline; Write-Host $t }
  function Warn($t) { Write-Host "  ! " -ForegroundColor Yellow -NoNewline; Write-Host $t }
  function Note($t) { Write-Host "  $t" -ForegroundColor DarkGray }
  function Row($label, $value) { Write-Host ("  " + $label.PadRight(11)) -ForegroundColor DarkGray -NoNewline; Write-Host $value }

  # Fail PROBLEM [WHAT TO DO...]
  function Fail($problem, [string[]]$hints = @()) {
    Write-Host ""
    Write-Host "  $Cross $problem" -ForegroundColor Red
    foreach ($h in $hints) { Write-Host "    $h" }
    Write-Host ""
    throw $Stop
  }

  function Show-Log([int]$lines = 12) {
    if (-not $Ctx.Log) { return }
    Write-Host ""
    $Ctx.Log -split "`r?`n" | Where-Object { $_ } | Select-Object -Last $lines | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
  }

  function Pretty($path) {
    if ($path -eq $HOME) { return "~" }
    if ($path.StartsWith($HOME + [IO.Path]::DirectorySeparatorChar)) { return "~" + $path.Substring($HOME.Length) }
    return $path
  }

  function Elapsed($start) {
    $s = [int]((Get-Date) - $start).TotalSeconds
    if ($s -lt 3) { return "" }
    if ($s -lt 60) { return "${s}s" }
    return "{0}m {1:00}s" -f [math]::Floor($s / 60), ($s % 60)
  }

  function Clear-Line {
    $width = 80
    try { $width = [Math]::Max([Console]::WindowWidth - 1, 1) } catch { }
    Write-Host ("`r" + (" " * $width) + "`r") -NoNewline
  }

  function Set-Cursor([bool]$visible) { try { [Console]::CursorVisible = $visible } catch { } }

  # Spin LABEL TEST [SECONDS]: a spinner beside LABEL until TEST passes (checked
  # every 2 seconds) or time runs out. Returns whether TEST passed.
  function Spin($label, [scriptblock]$test, [int]$seconds = 3600) {
    $start = Get-Date; $next = $start; $i = 0
    Set-Cursor $false
    try {
      while ($true) {
        if ((Get-Date) -ge $next) {
          if (& $test) { return $true }
          if (((Get-Date) - $start).TotalSeconds -ge $seconds) { return $false }
          $next = (Get-Date).AddSeconds(2)
        }
        Write-Host ("`r  " + $Frames[$i % $Frames.Count] + " ") -ForegroundColor DarkYellow -NoNewline
        Write-Host "$label  " -NoNewline
        Write-Host (Elapsed $start) -ForegroundColor DarkGray -NoNewline
        $i++
        Start-Sleep -Milliseconds 100
      }
    } finally { Clear-Line; Set-Cursor $true }
  }

  # Run LABEL ARGS...: runs docker with ARGS behind a spinner, its output in $Ctx.Log.
  function Run($label, [string[]]$arguments) {
    $out = Join-Path $Tmp "out.txt"; $err = Join-Path $Tmp "err.txt"
    # Windows PowerShell joins arguments with spaces and quotes nothing.
    $joined = ($arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }) -join " "
    $p = Start-Process -FilePath $Ctx.Docker -ArgumentList $joined -NoNewWindow -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $p.Handle # without it, ExitCode reads empty
    $null = Spin $label { $p.HasExited }
    $p.WaitForExit()
    $Ctx.Log = ((Get-Content $out, $err -ErrorAction SilentlyContinue) -join "`n")
    return $p.ExitCode -eq 0
  }

  # Quiet docker for questions: its output, or nothing when it fails.
  function D {
    $ErrorActionPreference = "Continue"
    $out = & $Ctx.Docker @args 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    return $out
  }

  function Confirm-Choice($question, [bool]$default) {
    if (-not $Interactive) { return $default }
    $hint = if ($default) { "[Y/n]" } else { "[y/N]" }
    Write-Host "  ? " -ForegroundColor DarkYellow -NoNewline
    Write-Host "$question " -NoNewline
    Write-Host "$hint " -ForegroundColor DarkGray -NoNewline
    $reply = Read-Host
    if (-not $reply) { return $default }
    return $reply -match "^[Yy]"
  }

  # --- Docker ------------------------------------------------------------------

  function Find-Docker {
    $cmd = Get-Command docker -ErrorAction SilentlyContinue
    if ($cmd) { $Ctx.Docker = $cmd.Source; return $true }
    # Docker Desktop was just installed and this window's PATH predates it.
    $bin = Join-Path $env:ProgramFiles "Docker\Docker\resources\bin"
    if ($IsWin -and (Test-Path (Join-Path $bin "docker.exe"))) {
      $env:Path = "$bin;$env:Path"
      $Ctx.Docker = Join-Path $bin "docker.exe"
      return $true
    }
    return $false
  }

  function Test-Docker { $null -ne (D info --format "{{.ServerVersion}}") }

  function Install-Docker {
    Warn "Nuvora runs in Docker Desktop, which isn't installed."
    if ($IsWin -and (Get-Command winget -ErrorAction SilentlyContinue) -and (Confirm-Choice "Install it now with winget?" $true)) {
      Say "Windows may ask for permission to install it."
      & winget install -e --id Docker.DockerDesktop --accept-package-agreements --accept-source-agreements
      if ($LASTEXITCODE -ne 0) { Fail "Docker Desktop didn't install." @("Get it from $DesktopUrl, then run this again.") }
      Ok "Installed Docker Desktop"
    } else {
      if ($Interactive) { Start-Process $DesktopUrl }
      Say "Get it from $DesktopUrl"
    }
    Fail "Docker Desktop needs to start once before Nuvora can install." @(
      "Open Docker Desktop and follow its setup; it may ask you to restart Windows.",
      "Then run this again: $InstallCmd")
  }

  function Start-Docker {
    $exe = Join-Path $env:ProgramFiles "Docker\Docker\Docker Desktop.exe"
    if (-not ($IsWin -and (Test-Path $exe))) { Fail "Docker isn't running." @("Start Docker Desktop and run this again.") }
    Start-Process $exe
    if (-not (Spin "Starting Docker Desktop" { Test-Docker } 180)) {
      Fail "Docker Desktop didn't start within 3 minutes." @("Open it, wait until it says it's running, and run this again.")
    }
  }

  function Initialize-Docker([bool]$install) {
    if (-not (Find-Docker)) {
      if (-not $install) { Fail "Docker isn't installed, so there's no Nuvora to remove." }
      Install-Docker
    }
    if (-not (Test-Docker)) { Start-Docker }
    if ($null -eq (D compose version)) { Fail "Docker Compose is missing." @("Update Docker Desktop and run this again.") }
    Ok ("Docker " + (D version --format "{{.Server.Version}}") + " is running")
  }

  function Compose { D compose --project-directory $Dir @args }

  # The nuvora container's details, if there is one.
  function Get-Container {
    $json = D inspect nuvora
    if (-not $json) { return $null }
    return ($json | Out-String | ConvertFrom-Json)[0]
  }

  # The folder the nuvora container was started from, "-" when not by Compose.
  function Get-ContainerDir {
    $c = Get-Container
    if (-not $c) { return "" }
    $dir = $c.Config.Labels."com.docker.compose.project.working_dir"
    if ($dir) { return $dir } else { return "-" }
  }

  function Get-RunningPort {
    $line = D port nuvora 8640/tcp | Select-Object -First 1
    if ($line -match ":(\d+)$") { return [int]$Matches[1] }
    return 0
  }

  function Get-ImageId($image) { D image inspect --format "{{.Id}}" $image }

  # --- Settings ------------------------------------------------------------------

  function Get-Saved($key) {
    $file = Join-Path $Dir ".env"
    if (-not (Test-Path $file)) { return "" }
    $line = Get-Content $file | Where-Object { $_ -like "$key=*" } | Select-Object -Last 1
    if (-not $line) { return "" }
    return $line.Substring($key.Length + 1).Trim("'", '"')
  }

  function Resolve-Folder($typed) {
    $p = $typed.Trim().Trim('"', "'")
    if ($p -eq "~") { $p = $HOME } elseif ($p -match '^~[\\/]') { $p = Join-Path $HOME $p.Substring(2) }
    if (-not $p -or -not (Test-Path -LiteralPath $p -PathType Container)) { return $null }
    $full = (Resolve-Path -LiteralPath $p).ProviderPath
    if ($full.Length -gt 3) { $full = $full.TrimEnd("\", "/") }
    return $full
  }

  $VideoExts = ".mkv", ".mp4", ".m4v", ".avi", ".mov", ".webm", ".wmv", ".m2ts", ".mpg"

  function Measure-Videos($dir, [int]$depth) {
    (Get-ChildItem -LiteralPath $dir -Recurse -Depth $depth -File -ErrorAction SilentlyContinue |
      Where-Object { $VideoExts -contains $_.Extension.ToLower() } | Select-Object -First 1000 | Measure-Object).Count
  }

  function Format-Videos([int]$n) {
    if ($n -ge 1000) { return "1,000+ videos" }
    if ($n -eq 1) { return "1 video" }
    return "$n videos"
  }

  # Folders where media often lives, with how many videos each holds.
  function Find-MediaFolders {
    $dirs = @("$HOME\Videos", "$HOME\Movies", "$HOME\Media")
    if (-not $IsWin) { $dirs = @("$HOME/Media", "$HOME/Movies", "$HOME/Videos") }
    if ($IsWin) {
      foreach ($drive in Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue) {
        foreach ($name in "Media", "Movies", "Videos", "Shows", "TV", "Anime", "Plex") { $dirs += Join-Path $drive.Root $name }
      }
    }
    $found = foreach ($d in $dirs | Select-Object -Unique) {
      if (-not (Test-Path -LiteralPath $d -PathType Container)) { continue }
      $n = Measure-Videos $d 3
      if ($n -gt 0) { [pscustomobject]@{ Path = $d; Count = $n } }
    }
    return @($found | Sort-Object Count -Descending | Select-Object -First 5)
  }

  # The Windows folder picker, in front of the terminal.
  function Show-FolderPicker {
    try {
      Add-Type -AssemblyName System.Windows.Forms
      $owner = New-Object System.Windows.Forms.Form -Property @{ TopMost = $true; ShowInTaskbar = $false }
      $dialog = New-Object System.Windows.Forms.FolderBrowserDialog -Property @{ Description = "Where are your movies and shows?"; ShowNewFolderButton = $false }
      if ($dialog.ShowDialog($owner) -eq "OK") { return $dialog.SelectedPath }
    } catch { }
    return $null
  }

  function Read-Media {
    Write-Host ""
    Write-Host "  Where are your movies and shows?" -ForegroundColor White
    Note "Nuvora only reads this folder; your files stay as they are."
    Write-Host "  Looking for videos..." -ForegroundColor DarkGray -NoNewline
    $found = Find-MediaFolders
    Clear-Line
    if ($found.Count -gt 0) {
      $width = ($found | ForEach-Object { (Pretty $_.Path).Length } | Measure-Object -Maximum).Maximum
      Write-Host ""
      for ($i = 0; $i -lt $found.Count; $i++) {
        Write-Host ("    " + ($i + 1)) -ForegroundColor DarkYellow -NoNewline
        Write-Host ("  " + (Pretty $found[$i].Path).PadRight($width) + "  ") -NoNewline
        Write-Host (Format-Videos $found[$i].Count) -ForegroundColor DarkGray
      }
      Write-Host ""
    }
    $picker = $IsWin -and [Threading.Thread]::CurrentThread.ApartmentState -eq "STA"
    $what = if ($found.Count -gt 0) { "a number or a path" } else { "a path" }
    if ($picker) { Note "Press Enter to choose it in a window, or type $what." } else { Note "Type $what." }
    while ($true) {
      Write-Host "  ? " -ForegroundColor DarkYellow -NoNewline
      Write-Host "Folder: " -NoNewline
      $reply = Read-Host
      $n = 0
      if (-not $reply.Trim()) {
        if (-not $picker) { continue }
        $reply = Show-FolderPicker
        if (-not $reply) { continue }
        Write-Host "    $reply"
      } elseif ($found.Count -gt 0 -and [int]::TryParse($reply.Trim(), [ref]$n)) {
        if ($n -lt 1 -or $n -gt $found.Count) { Warn "Pick a number from 1 to $($found.Count), or type a path."; continue }
        $Ctx.MediaCount = $found[$n - 1].Count
        Write-Host ""
        return $found[$n - 1].Path
      }
      $folder = Resolve-Folder $reply
      if (-not $folder) { Warn "There's no folder at $reply. Try again."; continue }
      $count = Measure-Videos $folder 5
      if ($count -eq 0) {
        Warn "There are no videos in $(Pretty $folder)."
        if (-not (Confirm-Choice "Use it anyway?" $false)) { continue }
      }
      $Ctx.MediaCount = $count
      Write-Host ""
      return $folder
    }
  }

  function Select-Media {
    if ($Ctx.MediaArg) {
      $folder = Resolve-Folder $Ctx.MediaArg
      if ($folder) { Ok "Media folder: $(Pretty $folder)"; return $folder }
      if (-not $Interactive) { Fail "There's no folder at $($Ctx.MediaArg)." @("Pass the folder with your movies and shows: -Media D:\Media") }
      Warn "There's no folder at $($Ctx.MediaArg) anymore."
    }
    if (-not $Interactive) {
      Fail "The installer needs the folder with your movies and shows." @(
        "Run it in a terminal to choose one, or pass it:",
        "& ([scriptblock]::Create((irm https://getnuvora.online/install.ps1))) -Media D:\Media")
    }
    $folder = Read-Media
    Write-Host "  $Tick " -ForegroundColor Green -NoNewline
    Write-Host "Media folder: $(Pretty $folder) " -NoNewline
    Write-Host "($(Format-Videos $Ctx.MediaCount))" -ForegroundColor DarkGray
    return $folder
  }

  function Test-PortBusy([int]$p) {
    $client = New-Object System.Net.Sockets.TcpClient
    try { return $client.ConnectAsync("127.0.0.1", $p).Wait(300) -and $client.Connected } catch { return $false } finally { $client.Dispose() }
  }

  function Select-Port([int]$port) {
    $ours = Get-RunningPort
    if ($port -gt 0) {
      if ($port -gt 65535) { Fail "$port isn't a port number." @("Use a number from 1 to 65535, like -Port $DefaultPort.") }
      if ($port -ne $ours -and (Test-PortBusy $port)) { Fail "Another program is using port $port." @("Stop it, or give Nuvora another one with -Port $($port + 1).") }
      return $port
    }
    $port = $DefaultPort
    if ($port -ne $ours -and (Test-PortBusy $port)) {
      while (Test-PortBusy $port) { $port++ }
      Warn "Port $DefaultPort is taken, so Nuvora uses $port."
    }
    return $port
  }

  # Docker wants IANA names (Europe/Amsterdam); Windows has its own, which only newer .NET converts.
  function Get-TimeZoneName {
    $id = [TimeZoneInfo]::Local.Id
    if ($id -match "/" -or -not $IsWin) { return $id }
    $iana = $null
    try { if ([TimeZoneInfo]::TryConvertWindowsIdToIanaId($id, [ref]$iana)) { return $iana } } catch { }
    return "UTC"
  }

  function Write-Utf8($path, $text) { [IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false)) }

  function Write-Files($image, $media, [int]$port) {
    $null = New-Item -ItemType Directory -Force -Path (Join-Path $Dir "data")
    $envFile = Join-Path $Dir ".env"
    # Lines the installer doesn't manage, like NUVORA_TRUSTED_PROXY, stay as they are.
    $keep = @()
    if (Test-Path $envFile) { $keep = @(Get-Content $envFile | Where-Object { $_ -notmatch '^(NUVORA_IMAGE|NUVORA_MEDIA|NUVORA_PORT|NUVORA_LISTEN|TZ)=' }) }
    $lines = @("NUVORA_IMAGE=$image", "NUVORA_MEDIA=$($media.Replace('\', '/'))", "NUVORA_PORT=$port", "NUVORA_LISTEN=0.0.0.0", "TZ=$(Get-TimeZoneName)") + $keep
    Write-Utf8 $envFile (($lines -join "`n") + "`n")
    $compose = @'
# Written by the Nuvora installer, which rewrites it on every update. Settings
# go in .env next to it; for changes of your own, create compose.override.yaml
# here, which Docker Compose reads too.
services:
  nuvora:
    image: ${NUVORA_IMAGE:?}
    container_name: nuvora
    user: "${NUVORA_UID:-10001}:${NUVORA_GID:-10001}"
    ports:
      - "${NUVORA_LISTEN:-0.0.0.0}:${NUVORA_PORT:-8640}:8640"
    cap_drop: [ALL]
    security_opt: [no-new-privileges:true]
    volumes:
      - ./data:/data
      - ${NUVORA_MEDIA:?}:/media:ro
    environment:
      TZ: ${TZ:-UTC}
      NUVORA_TRUSTED_PROXY: ${NUVORA_TRUSTED_PROXY:-}
      NUVORA_HARDWARE_ENCODING: ${NUVORA_HARDWARE_ENCODING:-auto}
    restart: unless-stopped

'@
    Write-Utf8 (Join-Path $Dir "compose.yaml") $compose.Replace("`r`n", "`n")
  }

  # --- Running Nuvora -------------------------------------------------------------

  function Get-Health([int]$port) {
    if ($port -le 0) { return $null }
    try { return Invoke-RestMethod -Uri "http://127.0.0.1:$port/api/health" -TimeoutSec 2 } catch { return $null }
  }

  function Get-LanIp {
    try {
      $s = New-Object Net.Sockets.Socket([Net.Sockets.AddressFamily]::InterNetwork, [Net.Sockets.SocketType]::Dgram, [Net.Sockets.ProtocolType]::Udp)
      $s.Connect("1.1.1.1", 53)
      $ip = $s.LocalEndPoint.Address.ToString()
      $s.Dispose()
      return $ip
    } catch { return "localhost" }
  }

  function Install-Nuvora {
    $updating = Test-Path (Join-Path $Dir ".env")
    Write-Host ""
    Write-Host "  Nuvora  " -ForegroundColor DarkYellow -NoNewline
    if ($updating) { Write-Host "Updating $(Pretty $Dir)" -ForegroundColor DarkGray } else { Write-Host "Installing in $(Pretty $Dir)" -ForegroundColor DarkGray }
    Write-Host ""

    Initialize-Docker $true
    $other = Get-ContainerDir
    if ($other -eq "-") { Fail "There's already a container named nuvora." @("Remove it with: docker rm -f nuvora (its data stays where it is), then run this again.") }
    if ($other -and $other -ne $Dir) {
      Fail "Nuvora already runs from $(Pretty $other)." @(
        "To update it: & ([scriptblock]::Create((irm https://getnuvora.online/install.ps1))) -Dir '$other'",
        "To move it here, first remove that one: docker rm -f nuvora")
    }

    # A pinned version stays pinned across updates.
    $version = $Version
    if (-not $version) { $saved = Get-Saved "NUVORA_IMAGE"; if ($saved.StartsWith("${ImageRepo}:")) { $version = $saved.Substring($ImageRepo.Length + 1) } }
    if (-not $version) { $version = "latest" }
    $image = "${ImageRepo}:$version"

    if (-not $Ctx.MediaArg) { $Ctx.MediaArg = (Get-Saved "NUVORA_MEDIA") }
    $mediaDir = Select-Media
    $port = $Port
    if ($port -le 0) { [int]::TryParse((Get-Saved "NUVORA_PORT"), [ref]$port) | Out-Null }
    $port = Select-Port $port

    $oldVersion = (Get-Health (Get-RunningPort)).version
    $before = Get-ImageId $image
    if (-not (Run "Downloading Nuvora" @("pull", $image))) {
      Show-Log
      $hint = "Check the internet connection and run this again."
      if ($Ctx.Log -match "manifest unknown|not found") { $hint = "There's no Nuvora $version. Versions: https://github.com/IuCC123/nuvora/releases" }
      Fail "Couldn't download Nuvora." @($hint)
    }
    $newImage = -not $before -or $before -ne (Get-ImageId $image)
    if ($newImage) { Ok "Downloaded Nuvora" }

    Write-Files $image $mediaDir $port
    if (-not (Run "Starting Nuvora" @("compose", "--project-directory", $Dir, "up", "-d", "--remove-orphans"))) {
      Show-Log
      if ($Ctx.Log -match "port is already allocated|address already in use") { Fail "Another program took port $port." @("Give Nuvora another one with -Port $($port + 1).") }
      Fail "Nuvora didn't start." @("For its log, run: docker logs nuvora")
    }
    # A container that stopped or keeps restarting won't get healthy by waiting.
    $healthy = Spin "Starting Nuvora" {
      if ((Get-Health $port).status -eq "ok") { return $true }
      $Ctx.Crashed = (Get-Container).State.Status -in "exited", "dead", "restarting"
      return $Ctx.Crashed
    } 90
    if (-not $healthy -or $Ctx.Crashed) {
      Write-Host ""
      D logs --tail 15 nuvora | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkGray }
      Fail "Nuvora didn't start." @("Its log is above; for all of it, run: docker logs nuvora")
    }

    $health = Get-Health $port
    $v = $health.version
    if ($oldVersion -and $oldVersion -ne $v) { Ok "Updated Nuvora $oldVersion $To $v" }
    elseif ($updating -and -not $newImage) { Ok "Nuvora $v is up to date" }
    elseif ($updating) { Ok "Updated Nuvora to the newest $v" }
    else { Ok "Nuvora $v is running" }

    $url = "http://$(Get-LanIp):$port"
    $setup = $false
    try { $setup = (Invoke-RestMethod -Uri "http://127.0.0.1:$port/api/auth/state" -TimeoutSec 2).setup_required } catch { }
    Write-Host ""
    Write-Host "  Open " -NoNewline
    Write-Host $url -ForegroundColor DarkYellow -NoNewline
    if ($setup) { Write-Host " to create your account." } else { Write-Host "" }
    if ($setup -and -not $updating -and $IsWin -and $Interactive) {
      Start-Process $url
      Note "It's opening in your browser now."
    }
    Note "Phones, laptops and the TV app on your network use this address too."
    Write-Host ""
    Row "Folder" (Pretty $Dir)
    Row "Update" "run the same command again"
    Row "Logs" "docker logs -f nuvora"
    Row "Uninstall" "& ([scriptblock]::Create((irm https://getnuvora.online/install.ps1))) -Uninstall"
    Write-Host ""
  }

  function Uninstall-Nuvora {
    Write-Host ""
    Write-Host "  Nuvora  " -ForegroundColor DarkYellow -NoNewline
    Write-Host "Uninstalling from $(Pretty $Dir)" -ForegroundColor DarkGray
    Write-Host ""
    if (-not (Test-Path (Join-Path $Dir "compose.yaml"))) { Fail "Nuvora isn't installed in $(Pretty $Dir)." @("If it's somewhere else, add -Dir with that folder.") }
    Say "This stops Nuvora and removes it. Your movies and shows aren't touched."
    if ($Interactive) {
      Write-Host ""
      if (-not (Confirm-Choice "Remove Nuvora?" $false)) { Say "Nothing changed."; Write-Host ""; return }
    }
    Initialize-Docker $false
    if (-not (Run "Removing Nuvora" @("compose", "--project-directory", $Dir, "down", "--rmi", "all", "--remove-orphans"))) { Show-Log; Fail "Couldn't remove Nuvora." }
    Ok "Removed Nuvora"
    if ($Interactive -and (Confirm-Choice "Also delete its data: accounts, watch progress and settings?" $false)) {
      Remove-Item -Recurse -Force -LiteralPath (Join-Path $Dir "data"), (Join-Path $Dir ".env"), (Join-Path $Dir "compose.yaml") -ErrorAction SilentlyContinue
      if (-not (Get-ChildItem -LiteralPath $Dir -Force -ErrorAction SilentlyContinue)) { Remove-Item -LiteralPath $Dir -Force }
      Ok "Deleted its data"
    } else {
      Ok "Kept its data in $(Pretty $Dir). Install again to pick up where you left off."
    }
    Write-Host ""
  }

  $Dir = if ($Dir) { $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Dir) } else { Join-Path $HOME "nuvora" }
  $Dir = $Dir.TrimEnd("\", "/")
  try {
    # Without -Dir, work on the Nuvora that's already running, wherever it was installed.
    if (-not $Ctx.DirGiven -and -not (Test-Path (Join-Path $Dir ".env")) -and (Find-Docker) -and (Test-Docker)) {
      $other = Get-ContainerDir
      if ($other -and $other -ne "-" -and (Test-Path (Join-Path $other ".env"))) { $Dir = $other }
    }
    if ($Uninstall) { Uninstall-Nuvora } else { Install-Nuvora }
  } catch {
    if ($_.Exception.Message -ne $Stop) {
      Write-Host ""
      Write-Host "  $Cross $($_.Exception.Message)" -ForegroundColor Red
      Write-Host ""
    }
  } finally {
    Set-Cursor $true
    Remove-Item -Recurse -Force -LiteralPath $Tmp -ErrorAction SilentlyContinue
  }
}
