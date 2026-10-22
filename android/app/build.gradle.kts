plugins { id("com.android.application") }
android {
    namespace = "moe.meltype.rust"
    compileSdk = 35
    defaultConfig {
        applicationId = "moe.meltype.rust"
        minSdk = 26
        targetSdk = 35
        versionCode = 2
        versionName = "0.1.0-pre.2"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}
dependencies {
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
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
