package com.example.thecommunication;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.widget.TextView;
import android.widget.Toast;
import androidx.appcompat.app.AppCompatActivity;
import com.google.zxing.integration.android.IntentIntegrator;
import com.google.zxing.integration.android.IntentResult;
import com.journeyapps.barcodescanner.CaptureActivity;
import android.graphics.Bitmap;
import android.widget.ImageView;
import com.google.zxing.BarcodeFormat;
import com.google.zxing.WriterException;
import com.google.zxing.qrcode.QRCodeWriter;
import com.google.zxing.common.BitMatrix;
import android.graphics.Color;
import org.json.JSONObject;

public class QRActivity extends AppCompatActivity {

    private TextView infoTextView;
    private ImageView qrImageView;
    private String myPeerId;
    private String myRelayAddress;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_qr);

        // Get extras
        myPeerId = getIntent().getStringExtra("MY_PEER_ID");
        myRelayAddress = getIntent().getStringExtra("MY_RELAY_ADDRESS");

        // Set title
        if (getSupportActionBar() != null) {
            getSupportActionBar().setTitle("QR Code");
            getSupportActionBar().setDisplayHomeAsUpEnabled(true);
        }

        infoTextView = findViewById(R.id.infoTextView);
        qrImageView = findViewById(R.id.qrImageView);

        // Display info
        String info = "Peer ID: " + myPeerId + "\n\nRelay: " + myRelayAddress;
        infoTextView.setText(info);

        // Generate QR Code
        generateQRCode();

        // Setup scan button
        findViewById(R.id.buttonScanQR).setOnClickListener(v -> scanQRCode());
    }

    private void generateQRCode() {
        try {
            // Create JSON with both peer ID and relay address
            JSONObject qrData = new JSONObject();
            qrData.put("peerId", myPeerId);
            qrData.put("address", myRelayAddress);

            String qrContent = qrData.toString();

            QRCodeWriter writer = new QRCodeWriter();
            BitMatrix bitMatrix = writer.encode(qrContent, BarcodeFormat.QR_CODE, 512, 512);

            int width = bitMatrix.getWidth();
            int height = bitMatrix.getHeight();
            Bitmap bmp = Bitmap.createBitmap(width, height, Bitmap.Config.RGB_565);

            for (int x = 0; x < width; x++) {
                for (int y = 0; y < height; y++) {
                    bmp.setPixel(x, y, bitMatrix.get(x, y) ? Color.BLACK : Color.WHITE);
                }
            }

            qrImageView.setImageBitmap(bmp);

        } catch (WriterException | org.json.JSONException e) {
            android.util.Log.e("QRActivity", "Error generating QR code", e);
            Toast.makeText(this, "Failed to generate QR code", Toast.LENGTH_SHORT).show();
        }
    }

    private void scanQRCode() {
        IntentIntegrator integrator = new IntentIntegrator(this);
        integrator.setDesiredBarcodeFormats(IntentIntegrator.QR_CODE);
        integrator.setPrompt("Scan a QR Code");
        integrator.setCameraId(0);
        integrator.setBeepEnabled(true);
        integrator.setBarcodeImageEnabled(false);
        integrator.setCaptureActivity(CaptureActivity.class);
        integrator.initiateScan();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        IntentResult result = IntentIntegrator.parseActivityResult(requestCode, resultCode, data);
        if (result != null) {
            if (result.getContents() == null) {
                Toast.makeText(this, "Scan cancelled", Toast.LENGTH_SHORT).show();
            } else {
                handleScannedData(result.getContents());
            }
        } else {
            super.onActivityResult(requestCode, resultCode, data);
        }
    }

    private void handleScannedData(String contents) {
        try {
            // Try to parse as JSON
            JSONObject qrData = new JSONObject(contents);
            String peerId = qrData.getString("peerId");
            String address = qrData.optString("address", "");

            android.util.Log.d("QRActivity", "Scanned Peer ID: " + peerId);
            android.util.Log.d("QRActivity", "Scanned Address: " + address);

            // Return to MainActivity with results
            Intent returnIntent = new Intent();
            returnIntent.putExtra("SCANNED_PEER_ID", peerId);
            returnIntent.putExtra("SCANNED_ADDRESS", address);
            setResult(Activity.RESULT_OK, returnIntent);
            finish();

        } catch (org.json.JSONException e) {
            // Fallback: treat as plain peer ID
            android.util.Log.w("QRActivity", "Failed to parse as JSON, treating as plain peer ID", e);

            Intent returnIntent = new Intent();
            returnIntent.putExtra("SCANNED_PEER_ID", contents);
            returnIntent.putExtra("SCANNED_ADDRESS", "");
            setResult(Activity.RESULT_OK, returnIntent);
            finish();
        }
    }

    @Override
    public boolean onSupportNavigateUp() {
        finish();
        return true;
    }
}
