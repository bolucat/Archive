/**
 * WeatherService — fetch weather + location via Open-Meteo and ip-api.
 * No API key needed — both services are free and CORS-enabled for browsers.
 */

export interface WeatherData {
  city: string
  temperature: number
  condition: string
  icon: string
  windSpeed: number
  humidity: number
}

export interface WeatherRadioMood {
  id: 'rain' | 'morning' | 'night' | 'sunny'
  label: string
  keywords: RegExp
}

const METEO_URL = 'https://api.open-meteo.com/v1/forecast'
const GEO_URL = 'https://geocoding-api.open-meteo.com/v1/search'
const IP_URL = 'http://ip-api.com/json/'

const CACHE_KEY = 'boxplayer-weather-cache'
const CACHE_TTL = 30 * 60 * 1000 // 30min

function conditionIcon(code: number): string {
  if (code <= 3) return '☀️'
  if (code <= 48) return '☁️'
  if (code <= 57) return '🌧️'
  if (code <= 67) return '🌨️'
  if (code <= 77) return '❄️'
  if (code <= 82) return '🌧️'
  return '⛈️'
}

function conditionLabel(code: number): string {
  if (code <= 1) return '晴'
  if (code <= 3) return '多云'
  if (code <= 48) return '阴'
  if (code <= 57) return '小雨'
  if (code <= 67) return '雨夹雪'
  if (code <= 77) return '雪'
  if (code <= 82) return '阵雨'
  return '雷暴'
}

export function getWeatherRadioMood(weather: WeatherData | null, hour = new Date().getHours()): WeatherRadioMood {
  const text = `${weather?.condition || ''} ${weather?.city || ''}`.toLowerCase()
  if (/rain|雨|雪|snow/.test(text)) {
    return { id: 'rain', label: '雨雪柔和电台', keywords: /rain|雨|雪|piano|钢琴|轻音乐|纯音乐|lofi|ambient/i }
  }
  if (hour < 8) {
    return { id: 'morning', label: '清晨唤醒电台', keywords: /ambient|piano|lofi|sleep|morning|晨|早|钢琴|轻音乐|纯音乐/i }
  }
  if (hour >= 18) {
    return { id: 'night', label: '夜间城市电台', keywords: /live|jazz|city|night|夜|晚|周杰伦|陈奕迅/i }
  }
  return { id: 'sunny', label: '晴日精选电台', keywords: /pop|精选|best|hit|热|推荐|周杰伦|陈奕迅|五月天/i }
}

export async function fetchWeather(city?: string): Promise<WeatherData | null> {
  try {
    // Check cache
    const cached = localStorage.getItem(CACHE_KEY)
    if (cached) {
      const entry = JSON.parse(cached)
      if (Date.now() - entry.at < CACHE_TTL) return entry.data
    }

    let lat: number, lon: number, name: string

    if (city) {
      const geoRes = await fetch(`${GEO_URL}?name=${encodeURIComponent(city)}&count=1&language=zh`)
      const geo = await geoRes.json()
      const r = geo.results?.[0]
      if (!r) return null
      lat = r.latitude
      lon = r.longitude
      name = r.name || city
    } else {
      const ipRes = await fetch(IP_URL)
      const ip = await ipRes.json()
      lat = ip.lat || 31.23
      lon = ip.lon || 121.47
      name = ip.city || '上海'
    }

    const meteoRes = await fetch(
      `${METEO_URL}?latitude=${lat}&longitude=${lon}&current=temperature_2m,weather_code,wind_speed_10m,relative_humidity_2m&timezone=auto`
    )
    const meteo = await meteoRes.json()
    const current = meteo.current

    const data: WeatherData = {
      city: name,
      temperature: Math.round(current.temperature_2m),
      condition: conditionLabel(current.weather_code),
      icon: conditionIcon(current.weather_code),
      windSpeed: current.wind_speed_10m,
      humidity: current.relative_humidity_2m,
    }

    localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), data }))
    return data
  } catch {
    return null
  }
}
