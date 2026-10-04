package api

import (
	"github.com/matthewdeanmartin/nanacoin/nanacoin_go/internal/core"
	"io"
)

// One encoded record per HTTP worker, allocated when the server is created.
// Never grow this buffer on request. Text has already been copied here before
// the service lock is released, so ring slots can safely be reused during I/O.
const RecordBufferCount = 4

// Includes a purchase's listing and transaction with maximum-length escaped
// text and account names. A response must not run out of space after money moves.
const RecordBufferBytes = 2560

type recordBuffer struct {
	result     core.WriteResult
	data       [RecordBufferBytes]byte
	n          int
	jsonMemory [jsonBufSize]byte
	json       jsonw
}

func (b *recordBuffer) Write(p []byte) (int, error) {
	if len(p) > len(b.data)-b.n {
		return 0, io.ErrShortBuffer
	}
	copy(b.data[b.n:], p)
	b.n += len(p)
	return len(p), nil
}

func (b *recordBuffer) prepare(sw *streamWriter, fn func(*jsonw)) bool {
	if sw.err != nil {
		return false
	}
	b.n = 0
	b.json = newJSONW(b, b.jsonMemory[:])
	fn(&b.json)
	sw.err = b.json.done()
	return sw.err == nil
}

func (s *Server) releaseRecord(b *recordBuffer) {
	b.json = jsonw{}
	b.result = core.WriteResult{}
	b.n = 0
	s.freeRecords <- b
}

func (b *recordBuffer) send(sw *streamWriter, add func(func(*jsonw))) bool {
	add(func(j *jsonw) { j.raw(b.data[:b.n]) })
	return sw.err == nil
}
