export interface NewsSource {
  id: string;
  name: string;
  url: string;
  category: string;
  enabled: boolean;
}

export interface NewsArticle {
  sourceId: string;
  sourceName: string;
  guid: string;
  title: string;
  link: string;
  description: string;
  published: string;
}
