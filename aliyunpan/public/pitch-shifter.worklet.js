// Phase Vocoder Pitch Shifter AudioWorklet
// Combined from: fft.js, ola-processor.js, phase-vocoder.js
// Source: https://github.com/indutny/fft.js + https://github.com/olvb/phaze

function FFT(size) {
  this.size = size | 0
  if (this.size <= 1 || (this.size & (this.size - 1)) !== 0) {
    throw new Error('FFT size must be a power of two and bigger than 1')
  }
  this._csize = size << 1

  var table = new Array(this.size * 2)
  for (var i = 0; i < table.length; i += 2) {
    var angle = Math.PI * i / this.size
    table[i] = Math.cos(angle)
    table[i + 1] = -Math.sin(angle)
  }
  this.table = table

  var power = 0
  for (var t = 1; this.size > t; t <<= 1) power++
  this._width = power % 2 === 0 ? power - 1 : power

  this._bitrev = new Array(1 << this._width)
  for (var j = 0; j < this._bitrev.length; j++) {
    this._bitrev[j] = 0
    for (var shift = 0; shift < this._width; shift += 2) {
      var revShift = this._width - shift - 2
      this._bitrev[j] |= ((j >>> shift) & 3) << revShift
    }
  }
  this._out = null
  this._data = null
  this._inv = 0
}

FFT.prototype.fromComplexArray = function(complex, storage) {
  var res = storage || new Array(complex.length >>> 1)
  for (var i = 0; i < complex.length; i += 2) res[i >>> 1] = complex[i]
  return res
}

FFT.prototype.createComplexArray = function() {
  var res = new Array(this._csize)
  for (var i = 0; i < res.length; i++) res[i] = 0
  return res
}

FFT.prototype.toComplexArray = function(input, storage) {
  var res = storage || this.createComplexArray()
  for (var i = 0; i < res.length; i += 2) {
    res[i] = input[i >>> 1]
    res[i + 1] = 0
  }
  return res
}

FFT.prototype.completeSpectrum = function(spectrum) {
  var size = this._csize
  var half = size >>> 1
  for (var i = 2; i < half; i += 2) {
    spectrum[size - i] = spectrum[i]
    spectrum[size - i + 1] = -spectrum[i + 1]
  }
}

FFT.prototype.transform = function(out, data) {
  if (out === data) throw new Error('Input and output buffers must be different')
  this._out = out
  this._data = data
  this._inv = 0
  this._transform4()
  this._out = null
  this._data = null
}

FFT.prototype.realTransform = function(out, data) {
  if (out === data) throw new Error('Input and output buffers must be different')
  this._out = out
  this._data = data
  this._inv = 0
  this._realTransform4()
  this._out = null
  this._data = null
}

FFT.prototype.inverseTransform = function(out, data) {
  if (out === data) throw new Error('Input and output buffers must be different')
  this._out = out
  this._data = data
  this._inv = 1
  this._transform4()
  for (var i = 0; i < out.length; i++) out[i] /= this.size
  this._out = null
  this._data = null
}

FFT.prototype._transform4 = function() {
  var out = this._out
  var size = this._csize
  var width = this._width
  var step = 1 << width
  var len = (size / step) << 1
  var outOff, t
  var bitrev = this._bitrev

  if (len === 4) {
    for (outOff = 0, t = 0; outOff < size; outOff += len, t++) {
      this._singleTransform2(outOff, bitrev[t], step)
    }
  } else {
    for (outOff = 0, t = 0; outOff < size; outOff += len, t++) {
      this._singleTransform4(outOff, bitrev[t], step)
    }
  }

  var inv = this._inv ? -1 : 1
  var table = this.table
  for (step >>= 2; step >= 2; step >>= 2) {
    len = (size / step) << 1
    var quarterLen = len >>> 2
    for (outOff = 0; outOff < size; outOff += len) {
      var limit = outOff + quarterLen
      for (var i = outOff, k = 0; i < limit; i += 2, k += step) {
        var A = i, B = A + quarterLen, C = B + quarterLen, D = C + quarterLen
        var Ar = out[A], Ai = out[A + 1], Br = out[B], Bi = out[B + 1]
        var Cr = out[C], Ci = out[C + 1], Dr = out[D], Di = out[D + 1]
        var MAr = Ar, MAi = Ai
        var tableBr = table[k], tableBi = inv * table[k + 1]
        var MBr = Br * tableBr - Bi * tableBi
        var MBi = Br * tableBi + Bi * tableBr
        var tableCr = table[2 * k], tableCi = inv * table[2 * k + 1]
        var MCr = Cr * tableCr - Ci * tableCi
        var MCi = Cr * tableCi + Ci * tableCr
        var tableDr = table[3 * k], tableDi = inv * table[3 * k + 1]
        var MDr = Dr * tableDr - Di * tableDi
        var MDi = Dr * tableDi + Di * tableDr
        var T0r = MAr + MCr, T0i = MAi + MCi
        var T1r = MAr - MCr, T1i = MAi - MCi
        var T2r = MBr + MDr, T2i = MBi + MDi
        var T3r = inv * (MBr - MDr), T3i = inv * (MBi - MDi)
        out[A] = T0r + T2r; out[A + 1] = T0i + T2i
        out[B] = T1r + T3i; out[B + 1] = T1i - T3r
        out[C] = T0r - T2r; out[C + 1] = T0i - T2i
        out[D] = T1r - T3i; out[D + 1] = T1i + T3r
      }
    }
  }
}

FFT.prototype._singleTransform2 = function(outOff, off, step) {
  var out = this._out, data = this._data
  var evenR = data[off], evenI = data[off + 1]
  var oddR = data[off + step], oddI = data[off + step + 1]
  out[outOff] = evenR + oddR
  out[outOff + 1] = evenI + oddI
  out[outOff + 2] = evenR - oddR
  out[outOff + 3] = evenI - oddI
}

FFT.prototype._singleTransform4 = function(outOff, off, step) {
  var out = this._out, data = this._data
  var inv = this._inv ? -1 : 1
  var step2 = step * 2, step3 = step * 3
  var Ar = data[off], Ai = data[off + 1]
  var Br = data[off + step], Bi = data[off + step + 1]
  var Cr = data[off + step2], Ci = data[off + step2 + 1]
  var Dr = data[off + step3], Di = data[off + step3 + 1]
  var T0r = Ar + Cr, T0i = Ai + Ci
  var T1r = Ar - Cr, T1i = Ai - Ci
  var T2r = Br + Dr, T2i = Bi + Di
  var T3r = inv * (Br - Dr), T3i = inv * (Bi - Di)
  out[outOff] = T0r + T2r; out[outOff + 1] = T0i + T2i
  out[outOff + 2] = T1r + T3i; out[outOff + 3] = T1i - T3r
  out[outOff + 4] = T0r - T2r; out[outOff + 5] = T0i - T2i
  out[outOff + 6] = T1r - T3i; out[outOff + 7] = T1i + T3r
}

FFT.prototype._realTransform4 = function() {
  var out = this._out, size = this._csize
  var width = this._width, step = 1 << width
  var len = (size / step) << 1
  var outOff, t, bitrev = this._bitrev
  if (len === 4) {
    for (outOff = 0, t = 0; outOff < size; outOff += len, t++) {
      this._singleRealTransform2(outOff, bitrev[t] >>> 1, step >>> 1)
    }
  } else {
    for (outOff = 0, t = 0; outOff < size; outOff += len, t++) {
      this._singleRealTransform4(outOff, bitrev[t] >>> 1, step >>> 1)
    }
  }
  var inv = this._inv ? -1 : 1, table = this.table
  for (step >>= 2; step >= 2; step >>= 2) {
    len = (size / step) << 1
    var halfLen = len >>> 1, quarterLen = halfLen >>> 1, hquarterLen = quarterLen >>> 1
    for (outOff = 0; outOff < size; outOff += len) {
      for (var i = 0, k = 0; i <= hquarterLen; i += 2, k += step) {
        var A = outOff + i, B = A + quarterLen, C = B + quarterLen, D = C + quarterLen
        var Ar = out[A], Ai = out[A + 1], Br = out[B], Bi = out[B + 1]
        var Cr = out[C], Ci = out[C + 1], Dr = out[D], Di = out[D + 1]
        var MAr = Ar, MAi = Ai
        var tableBr = table[k], tableBi = inv * table[k + 1]
        var MBr = Br * tableBr - Bi * tableBi, MBi = Br * tableBi + Bi * tableBr
        var tableCr = table[2 * k], tableCi = inv * table[2 * k + 1]
        var MCr = Cr * tableCr - Ci * tableCi, MCi = Cr * tableCi + Ci * tableCr
        var tableDr = table[3 * k], tableDi = inv * table[3 * k + 1]
        var MDr = Dr * tableDr - Di * tableDi, MDi = Dr * tableDi + Di * tableDr
        var T0r = MAr + MCr, T0i = MAi + MCi, T1r = MAr - MCr, T1i = MAi - MCi
        var T2r = MBr + MDr, T2i = MBi + MDi
        var T3r = inv * (MBr - MDr), T3i = inv * (MBi - MDi)
        out[A] = T0r + T2r; out[A + 1] = T0i + T2i
        out[B] = T1r + T3i; out[B + 1] = T1i - T3r
        if (i === 0) {
          out[C] = T0r - T2r; out[C + 1] = T0i - T2i
          continue
        }
        if (i === hquarterLen) continue
        var ST0r = T1r, ST0i = -T1i, ST1r = T0r, ST1i = -T0i
        var ST2r = -inv * T3i, ST2i = -inv * T3r
        var ST3r = -inv * T2i, ST3i = -inv * T2r
        var SA = outOff + quarterLen - i, SB = outOff + halfLen - i
        out[SA] = ST0r + ST2r; out[SA + 1] = ST0i + ST2i
        out[SB] = ST1r + ST3i; out[SB + 1] = ST1i - ST3r
      }
    }
  }
}

FFT.prototype._singleRealTransform2 = function(outOff, off, step) {
  var out = this._out, data = this._data
  var evenR = data[off], oddR = data[off + step]
  out[outOff] = evenR + oddR; out[outOff + 1] = 0
  out[outOff + 2] = evenR - oddR; out[outOff + 3] = 0
}

FFT.prototype._singleRealTransform4 = function(outOff, off, step) {
  var out = this._out, data = this._data
  var inv = this._inv ? -1 : 1
  var step2 = step * 2, step3 = step * 3
  var Ar = data[off], Br = data[off + step]
  var Cr = data[off + step2], Dr = data[off + step3]
  var T0r = Ar + Cr, T1r = Ar - Cr
  var T2r = Br + Dr, T3r = inv * (Br - Dr)
  out[outOff] = T0r + T2r; out[outOff + 1] = 0
  out[outOff + 2] = T1r; out[outOff + 3] = -T3r
  out[outOff + 4] = T0r - T2r; out[outOff + 5] = 0
  out[outOff + 6] = T1r; out[outOff + 7] = T3r
}

// ============ OLAProcessor ============

var WEBAUDIO_BLOCK_SIZE = 128

var OLAProcessor = class extends globalThis.AudioWorkletProcessor {
  constructor(options) {
    super(options)
    this.keepReturnTrue = true
    this.processNow = false
    this.nbInputs = options.numberOfInputs
    this.nbOutputs = options.numberOfOutputs
    this.blockSize = options.processorOptions.blockSize
    this.hopSize = WEBAUDIO_BLOCK_SIZE
    this.nbOverlaps = this.blockSize / this.hopSize
    this.lastSilencedHopCount = 0
    this.nbOverlaps2x = this.nbOverlaps * 2
    this.fakeEmptyInputs = [new Array(2).fill(new Float32Array(WEBAUDIO_BLOCK_SIZE))]

    this.inputBuffers = new Array(this.nbInputs)
    this.inputBuffersHead = new Array(this.nbInputs)
    this.inputBuffersToSend = new Array(this.nbInputs)
    for (var i = 0; i < this.nbInputs; i++) {
      this.allocateInputChannels(i, 2)
    }
    this.outputBuffers = new Array(this.nbOutputs)
    this.outputBuffersToRetrieve = new Array(this.nbOutputs)
    for (var i = 0; i < this.nbOutputs; i++) {
      this.allocateOutputChannels(i, 2)
    }
    this.port.onmessage = (e) => this.keepReturnTrue = false
  }

  allocateInputChannels(inputIndex, nbChannels) {
    this.inputBuffers[inputIndex] = new Array(nbChannels)
    for (var i = 0; i < nbChannels; i++) {
      this.inputBuffers[inputIndex][i] = new Float32Array(this.blockSize + WEBAUDIO_BLOCK_SIZE)
      this.inputBuffers[inputIndex][i].fill(0)
    }
    this.inputBuffersHead[inputIndex] = new Array(nbChannels)
    this.inputBuffersToSend[inputIndex] = new Array(nbChannels)
    for (var i = 0; i < nbChannels; i++) {
      this.inputBuffersHead[inputIndex][i] = this.inputBuffers[inputIndex][i].subarray(0, this.blockSize)
      this.inputBuffersToSend[inputIndex][i] = new Float32Array(this.blockSize)
    }
  }

  allocateOutputChannels(outputIndex, nbChannels) {
    this.outputBuffers[outputIndex] = new Array(nbChannels)
    for (var i = 0; i < nbChannels; i++) {
      this.outputBuffers[outputIndex][i] = new Float32Array(this.blockSize)
      this.outputBuffers[outputIndex][i].fill(0)
    }
    this.outputBuffersToRetrieve[outputIndex] = new Array(nbChannels)
    for (var i = 0; i < nbChannels; i++) {
      this.outputBuffersToRetrieve[outputIndex][i] = new Float32Array(this.blockSize)
      this.outputBuffersToRetrieve[outputIndex][i].fill(0)
    }
  }

  readInputs(inputs) {
    for (var i = 0; i < this.nbInputs; i++) {
      for (var j = 0; j < this.inputBuffers[i].length; j++) {
        var webAudioBlock = inputs[i][j]
        if (this.inputBuffers[i][j]) this.inputBuffers[i][j].set(webAudioBlock, this.blockSize)
      }
    }
  }

  shiftInputBuffers() {
    for (var i = 0; i < this.nbInputs; i++) {
      for (var j = 0; j < this.inputBuffers[i].length; j++) {
        this.inputBuffers[i][j].copyWithin(0, WEBAUDIO_BLOCK_SIZE)
      }
    }
  }

  prepareInputBuffersToSend() {
    for (var i = 0; i < this.nbInputs; i++) {
      for (var j = 0; j < this.inputBuffers[i].length; j++) {
        this.inputBuffersToSend[i][j].set(this.inputBuffersHead[i][j])
      }
    }
  }

  handleOutputBuffersToRetrieve() {
    for (var i = 0; i < this.nbOutputs; i++) {
      for (var j = 0; j < this.outputBuffers[i].length; j++) {
        for (var k = 0; k < this.blockSize; k++) {
          this.outputBuffers[i][j][k] += this.outputBuffersToRetrieve[i][j][k] / this.nbOverlaps
        }
      }
    }
  }

  writeOutputs(outputs) {
    for (var i = 0; i < this.nbInputs; i++) {
      for (var j = 0; j < this.inputBuffers[i].length; j++) {
        var webAudioBlock = this.outputBuffers[i][j].subarray(0, WEBAUDIO_BLOCK_SIZE)
        if (outputs[i][j]) outputs[i][j].set(webAudioBlock)
      }
    }
  }

  shiftOutputBuffers() {
    for (var i = 0; i < this.nbOutputs; i++) {
      for (var j = 0; j < this.outputBuffers[i].length; j++) {
        this.outputBuffers[i][j].copyWithin(0, WEBAUDIO_BLOCK_SIZE)
        this.outputBuffers[i][j].subarray(this.blockSize - WEBAUDIO_BLOCK_SIZE).fill(0)
      }
    }
  }

  process(inputs, outputs, params) {
    if (inputs[0].length < 2) {
      if (this.lastSilencedHopCount < this.nbOverlaps2x) {
        this.lastSilencedHopCount++
        inputs = this.fakeEmptyInputs
        this.processNow = true
      } else {
        if (this.lastSilencedHopCount === this.nbOverlaps2x) {
          this.lastSilencedHopCount++
        }
        this.processNow = false
      }
    } else {
      this.lastSilencedHopCount = 0
      this.processNow = true
    }
    if (this.processNow) {
      this.readInputs(inputs)
      this.shiftInputBuffers()
      this.prepareInputBuffersToSend()
      this.processOLA(this.inputBuffersToSend, this.outputBuffersToRetrieve, params)
      this.handleOutputBuffersToRetrieve()
      this.writeOutputs(outputs)
      this.shiftOutputBuffers()
    }
    return this.keepReturnTrue
  }
}

// ============ PhaseVocoderProcessor ============

var DEFAULT_BUFFERED_BLOCK_SIZE = 4096

function genHannWindow(length) {
  var win = new Float32Array(length)
  for (var i = 0; i < length; i++) {
    win[i] = 0.8 * (1 - Math.cos(2 * Math.PI * i / length))
  }
  return win
}

var PhaseVocoderProcessor = class extends OLAProcessor {
  static get parameterDescriptors() {
    return [{
      name: 'pitchFactor',
      defaultValue: 1.0,
      automationRate: 'k-rate',
    }]
  }

  constructor(options) {
    if (!options.processorOptions) options.processorOptions = {}
    if (!options.processorOptions.blockSize) options.processorOptions.blockSize = DEFAULT_BUFFERED_BLOCK_SIZE
    super(options)
    this.fftSize = this.blockSize
    this.timeCursor = 0
    this.hannWindow = genHannWindow(this.blockSize)
    this.fft = new FFT(this.fftSize)
    this.freqComplexBuffer = this.fft.createComplexArray()
    this.freqComplexBufferShifted = this.fft.createComplexArray()
    this.timeComplexBuffer = this.fft.createComplexArray()
    this.magnitudes = new Float32Array(this.fftSize / 2 + 1)
    this.peakIndexes = new Int32Array(this.magnitudes.length)
    this.nbPeaks = 0
  }

  processOLA(inputs, outputs, parameters) {
    var pitchFactor = parameters.pitchFactor[0]
    for (var i = 0; i < this.nbInputs; i++) {
      for (var j = 0; j < inputs[i].length; j++) {
        var input = inputs[i][j]
        var output = outputs[i][j]
        this.applyHannWindow(input)
        this.fft.realTransform(this.freqComplexBuffer, input)
        this.computeMagnitudes()
        this.findPeaks()
        this.shiftPeaks(pitchFactor)
        this.fft.completeSpectrum(this.freqComplexBufferShifted)
        this.fft.inverseTransform(this.timeComplexBuffer, this.freqComplexBufferShifted)
        this.fft.fromComplexArray(this.timeComplexBuffer, output)
        this.applyHannWindow(output)
      }
    }
    this.timeCursor += this.hopSize
  }

  applyHannWindow(input) {
    for (var i = 0; i < this.blockSize; i++) {
      input[i] = input[i] * this.hannWindow[i]
    }
  }

  computeMagnitudes() {
    var i = 0, j = 0
    while (i < this.magnitudes.length) {
      var real = this.freqComplexBuffer[j]
      var imag = this.freqComplexBuffer[j + 1]
      this.magnitudes[i] = real * real + imag * imag
      i += 1; j += 2
    }
  }

  findPeaks() {
    this.nbPeaks = 0
    var i = 2, end = this.magnitudes.length - 2
    while (i < end) {
      var mag = this.magnitudes[i]
      if (this.magnitudes[i - 1] >= mag || this.magnitudes[i - 2] >= mag) { i++; continue }
      if (this.magnitudes[i + 1] >= mag || this.magnitudes[i + 2] >= mag) { i++; continue }
      this.peakIndexes[this.nbPeaks] = i
      this.nbPeaks++
      i += 2
    }
  }

  shiftPeaks(pitchFactor) {
    this.freqComplexBufferShifted.fill(0)
    for (var i = 0; i < this.nbPeaks; i++) {
      var peakIndex = this.peakIndexes[i]
      var peakIndexShifted = Math.round(peakIndex * pitchFactor)
      if (peakIndexShifted > this.magnitudes.length) break
      var startIndex = 0, endIndex = this.fftSize
      if (i > 0) startIndex = peakIndex - Math.floor((peakIndex - this.peakIndexes[i - 1]) / 2)
      if (i < this.nbPeaks - 1) endIndex = peakIndex + Math.ceil((this.peakIndexes[i + 1] - peakIndex) / 2)
      var startOffset = startIndex - peakIndex, endOffset = endIndex - peakIndex
      for (var j = startOffset; j < endOffset; j++) {
        var binIndex = peakIndex + j, binIndexShifted = peakIndexShifted + j
        if (binIndexShifted >= this.magnitudes.length) break
        var omegaDelta = 2 * Math.PI * (binIndexShifted - binIndex) / this.fftSize
        var phaseShiftReal = Math.cos(omegaDelta * this.timeCursor)
        var phaseShiftImag = Math.sin(omegaDelta * this.timeCursor)
        var indexReal = binIndex * 2, indexImag = indexReal + 1
        var valueReal = this.freqComplexBuffer[indexReal]
        var valueImag = this.freqComplexBuffer[indexImag]
        var valueShiftedReal = valueReal * phaseShiftReal - valueImag * phaseShiftImag
        var valueShiftedImag = valueReal * phaseShiftImag + valueImag * phaseShiftReal
        var indexShiftedReal = binIndexShifted * 2, indexShiftedImag = indexShiftedReal + 1
        this.freqComplexBufferShifted[indexShiftedReal] += valueShiftedReal
        this.freqComplexBufferShifted[indexShiftedImag] += valueShiftedImag
      }
    }
  }
}

globalThis.registerProcessor('phase-vocoder-processor', PhaseVocoderProcessor)
