plugins { id("com.android.application") }
android {
    namespace = "moe.meltype.rust"
    compileSdk = 35
    defaultConfig {
        applicationId = "moe.meltype.rust"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0-rust-preview"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
val verifyNative by tasks.registering {
    doLast {
        for (abi in listOf("arm64-v8a", "x86_64")) {
            check(file("src/main/jniLibs/$abi/libmeltype_native.so").isFile) {
                "Missing Rust library for $abi. Run cargo ndk as described in android/README.md."
            }
        }
    }
}
tasks.named("preBuild") { dependsOn(verifyNative) }
