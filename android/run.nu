#!/usr/bin/env nu

# Run the Android app on a device and stream logcat output
def main [
    --device (-d): string  # Device type: "physical" or "virtual"
] {
    let package = "ly.hall.jetlagmobile"
    let activity = $"($package).GameScreen"

    # Get connected devices
    let devices = (adb devices | lines | skip 1 | where { $in != "" } | parse "{id}\t{status}" | where status == "device")

    # Categorize devices
    let physical_devices = ($devices | where { not ($in.id | str starts-with "emulator-") })
    let virtual_devices = ($devices | where { $in.id | str starts-with "emulator-" })

    # Determine which device to use
    let target_device = if $device != null {
        match $device {
            "physical" => {
                if ($physical_devices | is-empty) {
                    print "No physical device connected."
                    exit 1
                }
                $physical_devices | first | get id
            }
            "virtual" => {
                if ($virtual_devices | is-empty) {
                    print "No virtual device running. Starting emulator..."
                    start_emulator
                } else {
                    $virtual_devices | first | get id
                }
            }
            _ => {
                print $"Invalid device type: ($device). Use 'physical' or 'virtual'."
                exit 1
            }
        }
    } else {
        # Default: physical if available, otherwise virtual
        if not ($physical_devices | is-empty) {
            print "Using physical device (default)"
            $physical_devices | first | get id
        } else if not ($virtual_devices | is-empty) {
            print "Using virtual device (no physical device found)"
            $virtual_devices | first | get id
        } else {
            print "No devices available. Starting emulator..."
            start_emulator
        }
    }

    print $"Target device: ($target_device)"

    # Install the app
    print "Building and installing..."
    ./gradlew installDebug -PtargetDevice=($target_device)

    # Launch the app
    print "Launching app..."
    adb -s $target_device shell am start -n $"($package)/($activity)"

    # Get the app's PID for filtered logcat
    sleep 1sec
    let pid = (adb -s $target_device shell pidof $package | str trim)

    if $pid == "" {
        print "Warning: Could not get app PID, showing unfiltered logcat"
        adb -s $target_device logcat
    } else {
        print $"Streaming logcat for PID ($pid)... \(Ctrl+C to stop\)"
        adb -s $target_device logcat -v color $"--pid=($pid)"
    }
}

# Get the path to the emulator command
def get_emulator_path [] {
    # Try emulator directly first (in case it's in PATH)
    let direct = (which emulator | get path.0? | default null)
    if $direct != null {
        return "emulator"
    }
    
    # Try to find Android SDK location
    let android_home = $env.ANDROID_HOME? | default (
        $env.ANDROID_SDK_ROOT? | default null
    )
    
    if $android_home != null {
        let emulator_path = if $nu.os-info.name == "windows" {
            $"($android_home)\\emulator\\emulator.exe"
        } else {
            $"($android_home)/emulator/emulator"
        }
        
        if ($emulator_path | path exists) {
            return $emulator_path
        }
    }
    
    # Try common Windows location
    if $nu.os-info.name == "windows" {
        let user_home = $env.USERPROFILE
        let common_path = $"($user_home)\\AppData\\Local\\Android\\Sdk\\emulator\\emulator.exe"
        if ($common_path | path exists) {
            return $common_path
        }
    }
    
    print "ERROR: Could not find Android emulator."
    print "Please ensure Android SDK is installed and either:"
    print "  1. Add the emulator to your PATH, or"
    print "  2. Set ANDROID_HOME or ANDROID_SDK_ROOT environment variable"
    exit 1
}

# Start an Android emulator and wait for it to boot
def start_emulator [] {
    let emulator_cmd = get_emulator_path
    
    # Get list of available AVDs
    let avds = (^$emulator_cmd -list-avds | lines | where { $in != "" })
    
    if ($avds | is-empty) {
        print "No Android Virtual Devices (AVDs) found."
        print "Create one using Android Studio's AVD Manager."
        exit 1
    }
    
    # Use the first available AVD
    let avd_name = $avds | first
    print $"Starting emulator: ($avd_name)"
    
    # Start emulator in background (detached process)
    if $nu.os-info.name == "windows" {
        # On Windows, use cmd /c start to launch detached
        cmd /c start /B $emulator_cmd -avd $avd_name -no-snapshot-load
    } else {
        # On Unix-like systems, use nohup
        bash -c $"nohup ($emulator_cmd) -avd ($avd_name) -no-snapshot-load > /dev/null 2>&1 &"
    }
    
    # Wait for emulator to be detected by adb
    print "Waiting for emulator to boot..."
    mut booted = false
    mut attempts = 0
    let max_attempts = 60  # 60 seconds timeout
    
    while not $booted and $attempts < $max_attempts {
        sleep 1sec
        let devices = (adb devices | lines | skip 1 | where { $in != "" } | parse "{id}\t{status}")
        let emulator_devices = ($devices | where { $in.id | str starts-with "emulator-" })
        
        if not ($emulator_devices | is-empty) {
            let device_id = $emulator_devices | first | get id
            # Check if boot is complete
            let boot_status = (adb -s $device_id shell getprop sys.boot_completed | str trim)
            if $boot_status == "1" {
                $booted = true
                print $"Emulator ready: ($device_id)"
                return $device_id
            }
        }
        $attempts = $attempts + 1
    }
    
    if not $booted {
        print "Timeout waiting for emulator to boot."
        exit 1
    }
}