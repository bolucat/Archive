export type PosterAction = 'play' | 'loop' | 'shuffle' | 'favorite' | 'refresh' | 'watched' | 'playlist' | 'delete' | 'transcode' | 'rating' | 'share' | 'download' | 'select' | 'metadata' | 'continue' | 'series'
export function mediaPosterActions(server: boolean, tv: boolean): PosterAction[] {
 if (server) return tv ? ['play','loop','shuffle','rating','favorite','refresh','watched','playlist','delete'] : ['play','loop','rating','share','download','refresh','watched','favorite','playlist','delete']
 return tv ? ['play','loop','shuffle','select','rating','watched','continue','playlist','series','delete'] : ['play','loop','select','rating','share','download','metadata','watched','continue','playlist','series','delete']
}
