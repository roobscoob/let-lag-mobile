#!/usr/bin/env nu

# Run the iOS app on a device via SSH to Mac Studio
def main [] {
    let mac_host = "shared-mac-studio"
    let remote_path = "/Users/user/Documents/jet-lag"
    let project_name = "jetlagios"
    let bundle_id = "ly.hall.jetlagios"
    let device_id = "00008101-0001719636F8001E"
    let local_root = ($env.FILE_PWD | path dirname)

    # Sync project to Mac Studio using rsync (diff-based transfer)
    print "Syncing project to Mac Studio..."
    cd $local_root
    
    # Use WSL rsync on Windows, native rsync elsewhere
    if $nu.os-info.name == "windows" {
        ^wsl rsync -e "ssh.exe" -avz --delete --exclude '.git' --exclude 'target' --exclude 'android/build' --exclude 'android/.gradle' --exclude '*.xcuserstate' "./" $"($mac_host):($remote_path)/"
    } else {
        ^rsync -avz --delete --exclude '.git' --exclude 'target' --exclude 'android/build' --exclude 'android/.gradle' --exclude '*.xcuserstate' -e ssh "./" $"($mac_host):($remote_path)/"
    }

    # Unlock keychain and build in single SSH session
    print "Unlocking keychain and building..."
    let keychain_password = (op read "op://Shared Secrets/7jweds5dpaymscka2ay7dbuce4/password" | str trim)
    let build_cmd = $"
        set -e
        echo 'Unlocking keychain...'
        security unlock-keychain -p '($keychain_password)' ~/Library/Keychains/login.keychain-db && echo 'Keychain unlocked'
        security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k '($keychain_password)' ~/Library/Keychains/login.keychain-db 2>&1 || echo 'Partition list warning \(may be ok)'
        echo 'Starting build...'
        cd '($remote_path)'
        xcodebuild -project ($project_name).xcodeproj -scheme ($project_name) -destination 'id=($device_id)' -derivedDataPath build -allowProvisioningUpdates build
    "
    ssh $mac_host $build_cmd

    # Install app on device
    print "Installing on device..."
    let app_path = $"($remote_path)/build/Build/Products/Debug-iphoneos/($project_name).app"
    ssh $mac_host $"xcrun devicectl device install app --device '($device_id)' '($app_path)'"

    # Launch app on device
    print "Launching app..."
    ssh $mac_host $"xcrun devicectl device process launch --device '($device_id)' --terminate-existing '($bundle_id)'"

    print "Done!"
}