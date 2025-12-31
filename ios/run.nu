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

    # Build Rust library for iOS and generate Swift bindings
    print "Building Rust library for iOS..."
    let rust_build_cmd = $"
        set -e
        cd '($remote_path)'

        # Ensure iOS targets are installed
        rustup target add aarch64-apple-ios aarch64-apple-ios-sim

        # Build for iOS device
        echo 'Building Rust for iOS device...'
        cargo build -p jet-lag-mobile --target aarch64-apple-ios --release

        # Build for iOS simulator
        echo 'Building Rust for iOS simulator...'
        cargo build -p jet-lag-mobile --target aarch64-apple-ios-sim --release

        # Create output directories
        mkdir -p ios/Frameworks
        mkdir -p ios/Generated

        # Create XCFramework
        echo 'Creating XCFramework...'
        rm -rf ios/Frameworks/JetLagMobile.xcframework
        xcodebuild -create-xcframework -library target/aarch64-apple-ios/release/libjet_lag_mobile.a -library target/aarch64-apple-ios-sim/release/libjet_lag_mobile.a -output ios/Frameworks/JetLagMobile.xcframework

        # Generate Swift bindings
        echo 'Generating Swift bindings...'
        cargo run -p uniffi-bindgen -- generate --library target/aarch64-apple-ios/release/libjet_lag_mobile.a --language swift --out-dir ios/Generated

        # Create module.modulemap for the FFI header
        echo 'Creating modulemap...'
        echo 'module jet_lag_mobileFFI { header \"jet_lag_mobileFFI.h\" export * }' > ios/Generated/module.modulemap

        echo 'Rust build complete!'
    "
    ssh $mac_host $rust_build_cmd

    # Generate Xcode project using XcodeGen
    print "Generating Xcode project..."
    let xcodegen_cmd = $"
        set -e
        cd '($remote_path)/ios'

        # Generate xcodeproj from project.yml
        echo 'Running xcodegen...'
        /opt/homebrew/bin/xcodegen generate
    "
    ssh $mac_host $xcodegen_cmd

    # Unlock keychain and build in single SSH session
    print "Unlocking keychain and building Xcode project..."
    let keychain_password = (op read "op://Shared Secrets/7jweds5dpaymscka2ay7dbuce4/password" | str trim)
    let build_cmd = $"
        set -e
        echo 'Unlocking keychain...'
        security unlock-keychain -p '($keychain_password)' ~/Library/Keychains/login.keychain-db && echo 'Keychain unlocked'
        security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k '($keychain_password)' ~/Library/Keychains/login.keychain-db 2>&1 || echo 'Partition list warning \(may be ok)'
        echo 'Starting Xcode build...'
        cd '($remote_path)/ios'
        xcodebuild -project ($project_name).xcodeproj -scheme ($project_name) -destination 'id=($device_id)' -derivedDataPath build -allowProvisioningUpdates build
    "
    ssh $mac_host $build_cmd

    # Install app on device
    print "Installing on device..."
    let app_path = $"($remote_path)/ios/build/Build/Products/Debug-iphoneos/($project_name).app"
    ssh $mac_host $"xcrun devicectl device install app --device '($device_id)' '($app_path)'"

    # Launch app on device
    print "Launching app..."
    ssh $mac_host $"xcrun devicectl device process launch --device '($device_id)' --terminate-existing '($bundle_id)'"

    print "Done!"
}