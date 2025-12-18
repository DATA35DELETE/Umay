package com.example.thecommunication;

public class Contact {
    private String name;
    private String peerId;   // Yeni: Mesaj göndermek için gerekli ID (Örn: 12D3K...)
    private String address;  // Bağlantı (Dial) için gerekli Adres (Örn: /ip4/...)
    private String lastMessage;
    private String time;

    // Constructor güncellendi
    public Contact(String name, String peerId, String address, String lastMessage, String time) {
        this.name = name;
        this.peerId = peerId;
        this.address = address;
        this.lastMessage = lastMessage;
        this.time = time;
    }

    public String getName() { return name; }
    public String getPeerId() { return peerId; } // Yeni getter
    public String getAddress() { return address; }
    public String getLastMessage() { return lastMessage; }
    public String getTime() { return time; }

    // Mesaj geldiğinde arayüzü güncellemek için setter
    public void setLastMessage(String msg, String time) {
        this.lastMessage = msg;
        this.time = time;
    }
}