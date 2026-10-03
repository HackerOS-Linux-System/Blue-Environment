plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// CI passes the version (see build-blue-connect.yml); local builds get a dev default.
val bcVersionName = System.getenv("BC_VERSION_NAME") ?: "0.1.0"
val bcVersionCode = (System.getenv("BC_VERSION_CODE") ?: "1").toInt()

// Release signing is optional: only when all four variables are present (CI
// takes them from repository secrets). Without them `assembleRelease` yields an
// unsigned APK and `assembleDebug` is signed with the debug key as usual.
val ksPath: String? = System.getenv("BC_KEYSTORE_PATH")
val ksPassword: String? = System.getenv("BC_KEYSTORE_PASSWORD")
val bcKeyAlias: String? = System.getenv("BC_KEY_ALIAS")
val bcKeyPassword: String? = System.getenv("BC_KEY_PASSWORD")
val canSignRelease = listOf(ksPath, ksPassword, bcKeyAlias, bcKeyPassword).all { !it.isNullOrBlank() }

android {
    namespace = "org.legendaryos.blueconnect"
    compileSdk = 34

    defaultConfig {
        applicationId = "org.legendaryos.blueconnect"
        minSdk = 26
        targetSdk = 34
        versionCode = bcVersionCode
        versionName = bcVersionName
    }

    signingConfigs {
        if (canSignRelease) {
            create("release") {
                storeFile = file(ksPath!!)
                storePassword = ksPassword
                keyAlias = bcKeyAlias
                keyPassword = bcKeyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (canSignRelease) signingConfig = signingConfigs.getByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    buildFeatures { compose = true }

    sourceSets["main"].java.srcDir("src/main/kotlin")
    sourceSets["test"].java.srcDir("src/test/kotlin")

    testOptions { unitTests.isReturnDefaultValues = true }

    lint { abortOnError = false }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")

    val composeBom = platform("androidx.compose:compose-bom:2024.09.03")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")

    testImplementation("junit:junit:4.13.2")
    // android.jar's org.json is a stub on the JVM; the real one makes the protocol tests run.
    testImplementation("org.json:json:20240303")
}
