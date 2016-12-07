// The current protocol includes a 64 bit message size
// We should change it to 32 bit, but in the meantime just ignore the high part
const net = require('net');
const WebSocketServer = require('ws').Server;

const server_port = 9010;
const lycan_host = 'localhost';
const lycan_port = 7777;

const wss = new WebSocketServer({ port: server_port });

wss.on('connection', (ws) => {
  console.log('New client connected');
  try {
    // Establish a connection with Lycan
    let lycan = net.connect({port: lycan_port, host: lycan_host}, () => {
      console.log('Connection established with Lycan');

      // Once connected, forward messages from the client to Lycan
      ws.on('message', (message) => {
        // We could verify message is a valid JSON ...

        let size = Buffer.byteLength(message, 'utf8');
        let buf = Buffer.allocUnsafe(size + 8);
        // Write the size of the message, followed by the message
        buf.writeUInt32LE(size, 0);
        buf.writeUInt32LE(0x0, 4);
        // Dirty double-copy
        Buffer.from(message).copy(buf, 8);
        lycan.write(buf);
      });

      ws.on('close', (code, message) => {
        console.log('Connection with client closed, code ', code, ' message ', message);
        // Currently no clean way to disconnect from Lycan
        //lycan.end();
        lycan.destroy();
      });
    });

    lycan.on('error', (err) => {
      console.log('Error on the Lycan socket: ', err);
      ws.close();
      lycan.destroy();
    });

    // Forward messages from Lycan
    let next_msg_size = null;
    lycan.on('readable', () => {
      while (true) {
        // Parse the size first
        if (null === next_msg_size) {
          let buf;
          if (null !== (buf = lycan.read(8))) {
            next_msg_size = buf.readUInt32LE(0);
          } else {
            return;
          }
        }

        // Then get the actual message
        let buf;
        if (null !== (buf = lycan.read(next_msg_size))) {
          let message = buf.toString('utf-8', 0, next_msg_size);
          // And forward it to the client
          try { ws.send(message); }
          catch (err) { console.log(err); }
          // Reset the size (so we read it again next iteration)
          next_msg_size = null;
        } else {
          return;
        }
      }
    });

    lycan.on('close', (had_error) => {
      console.log('Connection with Lycan closed');
      if (had_error) {
        console.log('The Lycan connection closed with an error');
      }
      ws.close();
    });
  } catch (err) {
    console.log('Error when handling client: ', err);
  }
});
