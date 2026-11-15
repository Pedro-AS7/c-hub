// Emulador e view de terminal do Termux (Apache 2.0, veja TERMUX-LICENSE.md), com o processo local
// (JNI) trocado por um TerminalTransport.
plugins {
    id("com.android.library")
}

android {
    namespace = "com.termux.view"
    compileSdk = 37
    defaultConfig {
        minSdk = 31
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    lint {
        abortOnError = false
    }
}

dependencies {
    implementation("androidx.annotation:annotation:1.11.0")
}
