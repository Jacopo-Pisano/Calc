plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.jacopopisano.xiso2xex"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.jacopopisano.xiso2xex"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            // Firma di debug: comoda per installare l'APK di release a mano.
            // Per la distribuzione sostituirla con una keystore propria.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
}

// Compila il core Rust (../../core) per le ABI Android con cargo-ndk e copia
// le librerie in src/main/jniLibs. Disattivabile con -PskipRust=true (ad es.
// quando le .so sono già state prodotte da un passo precedente della CI).
val buildRust by tasks.registering(Exec::class) {
    group = "build"
    description = "Compila libxiso.so con cargo-ndk"
    workingDir = rootProject.file("../core")
    commandLine(
        "cargo", "ndk",
        "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
        "--platform", "26",
        "-o", file("src/main/jniLibs").absolutePath,
        "build", "--release", "--lib",
    )
    onlyIf { !project.hasProperty("skipRust") }
}
tasks.named("preBuild") { dependsOn(buildRust) }

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.documentfile:documentfile:1.0.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
}
