import type {
  TmdbSearchResult,
  MediaLibraryItem,
  MovieItem,
  MediaLibraryTvSeriesItem,
  MovieItemResponse,
  TvSeriesItemResponse
} from '../types/media'
import Config from '../config'
import { mediaFileNormalizer, type NormalizedMediaFileDescriptor } from './mediaFileNormalizer'

const TMDB_BASE_URL = `${Config.BOXPLAYER_API_URL.replace(/\/+$/, '')}/api/tmdb`

async function fetchWithRetry(url: string, retries = 3, delayMs = 2000): Promise<Response> {
  for (let attempt = 0; attempt <= retries; attempt++) {
    const resp = await fetch(url)
    if (resp.ok) return resp
    if (resp.status === 429 && attempt < retries) {
      await new Promise(r => setTimeout(r, delayMs * Math.pow(2, attempt)))
      continue
    }
    throw new Error(`TMDB fetch failed: ${resp.status}`)
  }
  throw new Error(`TMDB fetch failed after ${retries} retries`)
}

export const TMDB_BASE = TMDB_BASE_URL
export const TMDB_BASE_URL_PROXY = `${TMDB_BASE_URL}/proxy`

export function tmdbImageUrl(path?: string | null, size: string = 'w500'): string {
  const imagePath = String(path || '').replace(/^\/+/, '')
  return imagePath ? `${TMDB_BASE_URL}/image/${size}/${imagePath}` : ''
}

export { TMDB_BASE_URL }

export class TmdbService {
  private static instance: TmdbService
  
  static getInstance(): TmdbService {
    if (!TmdbService.instance) {
      TmdbService.instance = new TmdbService()
    }
    return TmdbService.instance
  }

  async searchMovie(
    queryName: string,
    year?: string,
    tmdbId?: string,
    fileHash?: string,
    fileName?: string
  ): Promise<MovieItem | null> {
    try {
      const params = new URLSearchParams({
        language: 'zh-CN'
      })

      if (tmdbId) {
        params.append('tmdbId', tmdbId)
      } else {
        params.append('title', queryName)
      }

      if (year) {
        params.append('year', year)
      }
      if (fileHash) {
        params.append('md5', fileHash)
      }
      if (fileName) {
        params.append('fileName', fileName)
      }

      const response = await fetchWithRetry(`${TMDB_BASE_URL}/movie?${params}`)

      const data: MovieItemResponse = await response.json()
      return data.data || null
    } catch (error) {
      console.error('Error searching movie:', error)
      return null
    }
  }

  async getMovieByTmdbId(tmdbId: number | string): Promise<MovieItem | null> {
    return this.searchMovie('', undefined, String(tmdbId))
  }

  async searchMedia(query: string, isTVFirst = false): Promise<{ movies: TmdbSearchResult[]; tv: TmdbSearchResult[] }> {
    const params = new URLSearchParams({ query, isTVFirst: String(isTVFirst), language: 'zh-CN' })
    const response = await fetchWithRetry(`${TMDB_BASE_URL}/search?${params}`)
    const data = await response.json()
    return {
      movies: Array.isArray(data?.movies) ? data.movies.map((item: TmdbSearchResult) => ({ ...item, media_type: 'movie' as const })) : [],
      tv: Array.isArray(data?.tv) ? data.tv.map((item: TmdbSearchResult) => ({ ...item, media_type: 'tv' as const })) : []
    }
  }

  async getTvByTmdbId(tmdbId: number | string, season = 1): Promise<MediaLibraryTvSeriesItem | null> {
    try {
      const params = new URLSearchParams({ id: String(tmdbId), season: String(season), language: 'zh-CN' })
      const response = await fetchWithRetry(`${TMDB_BASE_URL}/tv/id?${params}`)
      const data: TvSeriesItemResponse = await response.json()
      return data.data || null
    } catch (error) {
      console.error('Error loading TV metadata:', error)
      return null
    }
  }

  async searchTV(
    queryName: string,
    season: number,
    year?: string,
    tmdbId?: string,
    fileHash?: string,
    fileName?: string
  ): Promise<MediaLibraryTvSeriesItem | null> {
    try {
      const params = new URLSearchParams({
        language: 'zh-CN',
        season: season.toString()
      })

      if (tmdbId) {
        params.append('tmdbId', tmdbId)
      } else {
        params.append('title', queryName)
      }

      if (year) {
        params.append('year', year)
      }
      if (fileHash) {
        params.append('md5', fileHash)
      }
      if (fileName) {
        params.append('fileName', fileName)
      }

      const response = await fetchWithRetry(`${TMDB_BASE_URL}/tv?${params}`)

      const data: TvSeriesItemResponse = await response.json()
      return data.data || null
    } catch (error) {
      console.error('Error searching TV show:', error)
      return null
    }
  }


  normalizeFileName(fileName: string, folderHint?: string): NormalizedMediaFileDescriptor {
    return mediaFileNormalizer.normalize(fileName, folderHint)
  }

  cleanFileName(fileName: string, folderHint?: string): string {
    const descriptor = this.normalizeFileName(fileName, folderHint)
    return descriptor.searchTitle || descriptor.cleanedTitle
  }

  parseYear(fileName: string): string | undefined {
    const year = this.normalizeFileName(fileName).releaseYear
    return year === undefined ? undefined : String(year)
  }

  parseSeasonEpisode(fileName: string): { season: number; episode: number } | null {
    const descriptor = this.normalizeFileName(fileName)
    return descriptor.seasonNumber === undefined || descriptor.episodeNumber === undefined
      ? null
      : { season: descriptor.seasonNumber, episode: descriptor.episodeNumber }
  }

  async matchMedia(fileName: string, filePath: string, normalized?: NormalizedMediaFileDescriptor): Promise<Partial<MediaLibraryItem> | null> {
    const descriptor = normalized || this.normalizeFileName(fileName, filePath)
    const cleanedName = descriptor.searchTitle || descriptor.cleanedTitle
    const year = descriptor.releaseYear === undefined ? undefined : String(descriptor.releaseYear)
    const seasonEpisode = descriptor.seasonNumber === undefined || descriptor.episodeNumber === undefined
      ? null
      : { season: descriptor.seasonNumber, episode: descriptor.episodeNumber }

    if (seasonEpisode) {
      // 可能是电视剧
      const tvResult = await this.searchTV(cleanedName, seasonEpisode.season, year)
      if (tvResult) {
        // 从current_season中找到匹配的集数
        let matchedEpisode = null
        if (tvResult.current_season?.episodes) {
          matchedEpisode = tvResult.current_season.episodes.find(ep => ep.episode_number === seasonEpisode.episode)
        }

        // 构造返回的媒体信息，包含完整的剧集结构
        const mediaItem: Partial<MediaLibraryItem> = {
          type: 'tv',
          name: tvResult.tv.name || tvResult.tv.original_name,
          overview: tvResult.tv.overview,
          posterUrl: tvResult.tv.poster_path ? tmdbImageUrl(tvResult.tv.poster_path) : undefined,
          backdropUrl: tvResult.tv.backdrop_path ? tmdbImageUrl(tvResult.tv.backdrop_path, 'original') : undefined,
          year: tvResult.tv.first_air_date?.substring(0, 4),
          rating: tvResult.tv.vote_average,
          genres: tvResult.tv.genres?.map(g => g.name) || [],
          credits: tvResult.tv.credits || tvResult.current_season?.credits,
          productionCountries: tvResult.tv.production_countries?.map(c => c.name) || [],
          tmdbId: tvResult.tv.id,
          imdbId: tvResult.tv.imdbId,
          tvdbId: tvResult.tv.tvdbId,
          // 包含季信息
          seasons: tvResult.current_season ? [{
            id: tvResult.current_season.id,
            seasonNumber: tvResult.current_season.season_number,
            name: tvResult.current_season.name,
            overview: tvResult.current_season.overview,
            posterPath: tvResult.current_season.poster_path,
            episodeCount: tvResult.current_season.episode_count || tvResult.current_season.episodes?.length || 0,
            airDate: tvResult.current_season.air_date,
            credits: tvResult.current_season.credits
          }] : undefined
        }

        return mediaItem
      }
    } else {
      // 可能是电影
      const movieResult = await this.searchMovie(cleanedName, year)
      if (movieResult) {
        return {
          type: 'movie',
          name: movieResult.title || movieResult.original_title,
          overview: movieResult.overview,
          posterUrl: movieResult.poster_path ? tmdbImageUrl(movieResult.poster_path) : undefined,
          backdropUrl: movieResult.backdrop_path ? tmdbImageUrl(movieResult.backdrop_path, 'original') : undefined,
          year: movieResult.release_date?.substring(0, 4),
          rating: movieResult.vote_average,
          genres: movieResult.genres?.map(g => g.name) || [],
          credits: movieResult.credits,
          productionCountries: movieResult.production_countries?.map(c => c.name) || [],
          collectionId: movieResult.belongs_to_collection?.id,
          collectionName: movieResult.belongs_to_collection?.name,
          tmdbId: movieResult.id,
          imdbId: movieResult.imdb_id,
          driveFiles: [] // 这个会在后续处理中填充实际的文件信息
        }
      }
    }

    return null
  }
}
