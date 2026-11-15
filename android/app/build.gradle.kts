plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.pedro.chub"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.pedro.chub"
        minSdk = 31
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Assinado com a chave de depuração: o app só é instalado por adb neste celular.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
    packaging {
        resources {
            // O BouncyCastle traz manifestos repetidos por versão do Java.
            excludes += setOf("META-INF/versions/**", "META-INF/*.md", "META-INF/LICENSE*", "META-INF/NOTICE*")
        }
    }
    lint {
        abortOnError = false
    }
}

dependencies {
    implementation(project(":terminal"))

    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.11.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.11.0")
    implementation("androidx.fragment:fragment-ktx:1.9.1")
    implementation("androidx.biometric:biometric:1.1.0")
    implementation("androidx.core:core-ktx:1.19.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")

    // SSH: sshj com o BouncyCastle completo (o do Android é reduzido).
    implementation("com.hierynomus:sshj:0.41.2")
    implementation("org.bouncycastle:bcprov-jdk18on:1.87")
    implementation("org.bouncycastle:bcpkix-jdk18on:1.87")
    implementation("org.slf4j:slf4j-nop:2.0.17")

    testImplementation("junit:junit:4.13.2")
    // O org.json do Android é só um esqueleto nos testes de unidade.
    testImplementation("org.json:json:20260814")
}
