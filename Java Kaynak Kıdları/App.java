package com.example.thecommunication;

import android.app.Application;

public class App extends Application {

    @Override
    public void onCreate() {
        super.onCreate();

        android.util.Log.d("App", "Application onCreate - No node initialization here");

        // Sadece loglama - Node başlatma MainActivity'de yapılıyor
    }
}