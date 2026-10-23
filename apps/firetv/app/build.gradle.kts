plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "app.nuvora.tv"
    compileSdk = 37

    defaultConfig {
        applicationId = "app.nuvora.tv"
        // Fire OS 6 (Fire TV Stick 2nd gen and newer) and every Android TV since 7.1.
        minSdk = 25
        targetSdk = 36
        versionCode = (project.findProperty("nuvora.versionCode") as String?)?.toInt() ?: 200
        versionName = (project.findProperty("nuvora.versionName") as String?) ?: "0.2.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        // Release builds are signed with the key in the environment when one is
        // given (CI), else with the debug key so a local release build installs.
        create("release") {
            val store = System.getenv("NUVORA_KEYSTORE")
            if (store != null) {
                storeFile = file(store)
                storePassword = System.getenv("NUVORA_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("NUVORA_KEY_ALIAS")
                keyPassword = System.getenv("NUVORA_KEY_PASSWORD")
            } else {
                val debug = signingConfigs.getByName("debug")
                storeFile = debug.storeFile
                storePassword = debug.storePassword
                keyAlias = debug.keyAlias
                keyPassword = debug.keyPassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        // FilterChip (library sorting, seasons) is still marked experimental.
        optIn.add("androidx.tv.material3.ExperimentalTvMaterial3Api")
    }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.tv.material)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.viewmodel.compose)
    implementation(libs.lifecycle.runtime.compose)
    implementation(libs.navigation.compose)
    implementation(libs.core.ktx)
    implementation(libs.media3.exoplayer)
    implementation(libs.media3.exoplayer.hls)
    implementation(libs.media3.datasource.okhttp)
    implementation(libs.media3.ui)
    implementation(libs.serialization.json)
    implementation(libs.coroutines.android)
    implementation(libs.okhttp)
    implementation(libs.coil.compose)
    implementation(libs.coil.okhttp)
    testImplementation(libs.junit)
    androidTestImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.runner)
    androidTestImplementation(libs.androidx.test.junit)
}
