package spootiferdb

import "log"

var writeChan = make(chan writeMessage)

type writeMessage struct {
	errTopic  chan error
	writeFunc func() error
}

func (w writeMessage) writeSync() error {
	writeChan <- w
	return <-w.errTopic
}

func (w writeMessage) writeAsync() {
	writeChan <- w

	go handleErr(w.errTopic)
}

// Writer
// We are using SQLite, which works best with a single writer process.
// Therefore, whenever performing a database write, we should use this
// interface to forward writes to the write thread.
type Writer interface {
	WriteSync(func() error) error
	Write()
}

func newWriteMessage(writeFunc func() error) writeMessage {
	errTopic := make(chan error, 1)

	return writeMessage{
		errTopic:  errTopic,
		writeFunc: writeFunc,
	}
}

func handleErr(errChan chan error) {
	err := <-errChan

	if err != nil {
		log.Println("Error processing write: ", err)
	}
}

// WriteSync
// Sends a function that should contain a database write to the write thread.
// Then, waits for the function to finish and returns the error.
func WriteSync(writeFunc func() error) error {
	msg := newWriteMessage(writeFunc)
	return msg.writeSync()
}

func WriteAsync(writeFunc func() error) {
	msg := newWriteMessage(writeFunc)

	msg.writeAsync()
}

func processWrites() {
	for msg := range writeChan {
		if msg.errTopic != nil {
			msg.errTopic <- msg.writeFunc()
			close(msg.errTopic)
		} else {
			err := msg.writeFunc()

			if err != nil {
				log.Println("Error processing write: ", err)
			}
		}

	}
}

// StartWriteThread
// We are using SQLite, and so we only support a single writer thread.
// All writes should be sent to the channel that is read from by the
// processWrites func.
func StartWriteThread() {
	go processWrites()
}
