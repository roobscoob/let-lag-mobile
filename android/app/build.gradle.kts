plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.rust.android)
}

android {
    namespace = "ly.hall.jetlagmobile"
    compileSdk = 35
    ndkVersion = "29.0.14206865"

    defaultConfig {
        applicationId = "ly.hall.jetlagmobile"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    buildFeatures {
        compose = true
    }

    sourceSets {
        getByName("main") {
            java.srcDir(layout.buildDirectory.dir("generated/uniffi"))
            jniLibs.srcDir(layout.buildDirectory.dir("rustJniLibs/android"))
        }
    }
}

val generateUniFFIBindings by tasks.registering(Exec::class) {
    val rustProjectDir = rootProject.projectDir.resolve("../crates/mobile")
    val workspaceDir = rootProject.projectDir.resolve("..")
    val bindgenDir = layout.buildDirectory.dir("generated/uniffi")

    workingDir = workspaceDir

    val isWindows = System.getProperty("os.name").lowercase().contains("windows")
    val libExtension = if (isWindows) "dll" else if (System.getProperty("os.name").lowercase().contains("mac")) "dylib" else "so"
    val libPath = workspaceDir.resolve("target/release/jet_lag_mobile.$libExtension")

    if (isWindows) {
        commandLine("cmd", "/c", "cargo run -p uniffi-bindgen -- generate --library $libPath --language kotlin --out-dir ${bindgenDir.get().asFile.absolutePath}")
    } else {
        commandLine("cargo", "run", "-p", "uniffi-bindgen", "--", "generate", "--library", libPath.absolutePath, "--language", "kotlin", "--out-dir", bindgenDir.get().asFile.absolutePath)
    }

    inputs.dir(rustProjectDir.resolve("src"))
    outputs.dir(bindgenDir)

    dependsOn("cargoBuild")
}

tasks.matching { it.name.startsWith("compile") && it.name.contains("Kotlin") }.configureEach {
    dependsOn(generateUniFFIBindings)
}

dependencies {
    implementation(libs.jna) { artifact { type = "aar" } }
    implementation(libs.maplibre.android)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.ui)
    implementation(libs.androidx.ui.graphics)
    implementation(libs.androidx.ui.tooling.preview)
    implementation(libs.androidx.material3)
    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.ui.test.junit4)
    debugImplementation(libs.androidx.ui.tooling)
    debugImplementation(libs.androidx.ui.test.manifest)
}

tasks.register<Exec>("uvSync") {
    workingDir = file(".")
    commandLine("uv", "sync")
}

cargo {
    module = "${projectDir}/../../crates/mobile"
    libname = "jet_lag_mobile"
    targets = listOf("arm", "arm64", "x86", "x86_64")
    profile = "release"
    targetDirectory = "${projectDir}/../../target"
    pythonCommand = if (System.getProperty("os.name").startsWith("Windows")) {
        "${projectDir}/../.venv/Scripts/python.exe"
    } else {
        "${projectDir}/../.venv/bin/python"
    }
}

tasks.configureEach {
    if (name.startsWith("cargoBuild")) {
        dependsOn("uvSync")
    }
    if (name.startsWith("merge") && name.endsWith("NativeLibs")) {
        dependsOn("cargoBuild")
    }
}