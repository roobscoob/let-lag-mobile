#!/usr/bin/env nu

# Run the Android app on a device and stream logcat output
def main [
    --device (-d): string  # Device type: "physical" or "virtual"
] {
    let package = "ly.hall.jetlagmobile"
    let activity = $"($package).GameScreen"

    # Get connected devices
    let devices = (adb devices | lines | skip 1 | where { $in != "" } | parse "{id}\t{status}" | where status == "device")

    if ($devices | is-empty) {
        print "No devices connected. Please connect a device or start an emulator."
        exit 1
    }

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
                    print "No virtual device (emulator) running."
                    exit 1
                }
                $virtual_devices | first | get id
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
            print "No devices available."
            exit 1
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
