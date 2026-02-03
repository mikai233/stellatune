plugins {
    id("com.android.application")
    id("kotlin-android")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// --- Rust/Android build integration (cargo-ndk) ---
val targetPlatform = project.findProperty("target-platform") as String?
val flutterPlatforms =
    (project.findProperty("flutter.targetPlatforms") as String?)?.split(",") ?: emptyList()
val abiSlug = targetPlatform
    ?: (if (flutterPlatforms.isNotEmpty()) flutterPlatforms.joinToString("-") else "universal")

// Isolated directory in 'build/' ensures separation between builds (universal vs arm64, etc).
val jniLibsOutDir = file("${layout.buildDirectory.get().asFile}/rustJniLibs/$abiSlug")

android {
    namespace = "io.github.mikai233.stellatune"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = JavaVersion.VERSION_17.toString()
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "io.github.mikai233.stellatune"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    // Use the isolated directory as the only JNI libs source. Rust build task writes into it.
    sourceSets["main"].jniLibs.setSrcDirs(listOf(jniLibsOutDir))

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

flutter {
    source = "../.."
}

val repoRootDir = file("../../../../") // from apps/stellatune/android/app -> repo root
val rustWorkspaceDir = repoRootDir
val rustPackageName = "stellatune-ffi" // Cargo package name (with hyphen)

fun cargoCmd(): List<String> {
    // Windows requires "cmd /c" to run cargo reliably from Gradle Exec.
    val isWindows = org.gradle.internal.os.OperatingSystem.current().isWindows
    return if (isWindows) listOf("cmd", "/c", "cargo") else listOf("cargo")
}

fun registerRustAndroidBuildTask(taskName: String, isRelease: Boolean) {
    tasks.register<Exec>(taskName) {
        group = "build"
        description = "Build Rust cdylib (.so) for Android ABIs and copy into isolated jniLibs (${if (isRelease) "Release" else "Debug"})"

        workingDir = rustWorkspaceDir
        outputs.dir(jniLibsOutDir)

        // Always run this task; let Cargo handle incremental compilation.
        outputs.upToDateWhen { false }

        doFirst {
            val requestedAbis = mutableListOf<String>()
            if (targetPlatform != null) {
                when (targetPlatform) {
                    "android-arm" -> requestedAbis.add("armeabi-v7a")
                    "android-arm64" -> requestedAbis.add("arm64-v8a")
                    "android-x64" -> requestedAbis.add("x86_64")
                }
            }

            if (requestedAbis.isEmpty()) {
                for (p in flutterPlatforms) {
                    when (p.trim()) {
                        "android-arm" -> requestedAbis.add("armeabi-v7a")
                        "android-arm64" -> requestedAbis.add("arm64-v8a")
                        "android-x64" -> requestedAbis.add("x86_64")
                    }
                }
            }

            val finalAbis = requestedAbis.ifEmpty {
                listOf("armeabi-v7a", "arm64-v8a", "x86_64")
            }
            val abiArgs = finalAbis.flatMap { listOf("-t", it) }

            logger.lifecycle(
                "Rust build ($abiSlug) targeting ABIs: $finalAbis -> ${jniLibsOutDir.absolutePath} (${if (isRelease) "release" else "debug"})",
            )

            if (jniLibsOutDir.exists()) {
                jniLibsOutDir.deleteRecursively()
            }
            jniLibsOutDir.mkdirs()

            val args = mutableListOf<String>()
            args += cargoCmd()
            args += listOf("ndk", "--platform", "21")
            args += abiArgs
            args += listOf("-o", jniLibsOutDir.absolutePath, "build")
            if (isRelease) {
                args += "--release"
            }
            args += listOf("-p", rustPackageName)

            commandLine(args)
        }
    }
}

registerRustAndroidBuildTask("buildRustAndroidSoDebug", isRelease = false)
registerRustAndroidBuildTask("buildRustAndroidSoRelease", isRelease = true)

// Ensure Rust is built before Android builds the APK/AAB.
// Desired mapping:
// - Flutter Debug   -> Rust Debug
// - Flutter Profile -> Rust Release
// - Flutter Release -> Rust Release
tasks.matching { it.name == "preDebugBuild" || it.name == "assembleDebug" }.configureEach {
    dependsOn("buildRustAndroidSoDebug")
}
tasks.matching {
    it.name == "preProfileBuild" || it.name == "assembleProfile" || it.name == "preReleaseBuild" || it.name == "assembleRelease"
}.configureEach {
    dependsOn("buildRustAndroidSoRelease")
}
