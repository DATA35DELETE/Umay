package com.example.thecommunication;

public class NativeLib {

    private static NativeLib instance;

    private NativeLib() {
        // Private constructor for singleton
    }

    public static NativeLib getInstance() {
        if (instance == null) {
            instance = new NativeLib();
        }
        return instance;
    }

    public static String init() {
        try {
            System.loadLibrary("theCommunication");
            return null;
        } catch (UnsatisfiedLinkError e) {
            return e.getMessage();
        } catch (Exception e) {
            return e.toString();
        }
    }

    // Mesajları dinlemek için bir Interface tanımlıyoruz
    public interface MessageListener {
        void onMessageReceived(String senderId, String message);
    }

    private static MessageListener listener;

    public static void setListener(MessageListener msgListener) {
        listener = msgListener;
    }

    // Rust tarafından çağrılacak metod
    // NOT: Bu metod static değil, instance method olmalı
    public void onMessageReceived(String senderId, String message) {
        android.util.Log.d("NativeLib", "onMessageReceived called: " + senderId + " - " + message);
        if (listener != null) {
            listener.onMessageReceived(senderId, message);
        } else {
            android.util.Log.w("NativeLib", "No listener set!");
        }
    }

    // Native metodlar
    public native void startNode(String seed, String identityPath);
    public native void dialPeer(String address);
    public native void sendMessage(String peerId, String message);
    public native void saveContact(String name, String address);
    public native void connectContact(String name);

    // Yeni metodlar: Peer ID ve Listen Adresleri
    public native String getMyPeerId();
    public native String[] getListenAddresses();
}