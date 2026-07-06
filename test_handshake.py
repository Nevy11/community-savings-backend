import hmac,hashlib,urllib.request
body = b'{ test:ok}'
sig = hmac.new(b'ee833caad28152c7ccce0899e9eac05f9891773ef4318b0652f57156e624ce60', body, hashlib.sha256).hexdigest()
req = urllib.request.Request('http://localhost:3000/api/webhooks/supabase-auth/handshake', data=body, headers={'Content-Type':'application/json','X-Hook-Signature':sig})
try:
    resp = urllib.request.urlopen(req)
    print(resp.status)
    print(resp.read().decode())
except Exception as e:
    print('ERROR', e)
