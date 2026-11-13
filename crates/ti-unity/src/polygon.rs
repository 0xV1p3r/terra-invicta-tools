//! Filled polygons on a 1-bit mask, ported from Pillow's `Draw.c` (`ImagingDrawPolygon` with
//! `fill`, 8-bit images) so sprite masks match `ImageDraw.polygon` exactly, including its
//! single-precision float arithmetic.

#[derive(Debug, Clone, Copy)]
struct Edge {
    x0: i32,
    y0: i32,
    xmin: i32,
    ymin: i32,
    xmax: i32,
    ymax: i32,
    dx: f32,
}

fn edge(x0: i32, y0: i32, x1: i32, y1: i32) -> Edge {
    let dx = if y0 == y1 { 0.0 } else { (x1 - x0) as f32 / (y1 - y0) as f32 };
    Edge {
        x0,
        y0,
        xmin: x0.min(x1),
        xmax: x0.max(x1),
        ymin: y0.min(y1),
        ymax: y0.max(y1),
        dx,
    }
}

/// `ROUND_UP` of `Draw.c`: `f + 0.5F` in float, then `floor`.
fn round_up(f: f32) -> i32 {
    if f >= 0.0 {
        f64::from(f + 0.5).floor() as i32
    } else {
        -(f64::from(f.abs() + 0.5).floor() as i32)
    }
}

fn round_down(f: f32) -> i32 {
    if f >= 0.0 {
        f64::from(f - 0.5).ceil() as i32
    } else {
        -(f64::from(f.abs() - 0.5).ceil() as i32)
    }
}

/// A 1-bit image, one byte per pixel.
pub(crate) struct Mask {
    pub width: i32,
    pub height: i32,
    pub data: Vec<u8>,
}

impl Mask {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width as i32,
            height: height as i32,
            data: vec![0; width as usize * height as usize],
        }
    }

    fn hline(&mut self, mut x0: i32, y: i32, mut x1: i32) {
        if y < 0 || y >= self.height {
            return;
        }
        if x0 < 0 {
            x0 = 0;
        } else if x0 >= self.width {
            return;
        }
        if x1 < 0 {
            return;
        } else if x1 >= self.width {
            x1 = self.width - 1;
        }
        if x0 <= x1 {
            let row = (y * self.width) as usize;
            self.data[row + x0 as usize..=row + x1 as usize].fill(1);
        }
    }

    /// `ImageDraw.polygon(xy, fill=1)`; Pillow truncates the coordinates to `int`.
    pub fn fill_polygon(&mut self, points: &[(f64, f64)]) {
        let xy: Vec<i32> = points.iter().flat_map(|&(x, y)| [x as i32, y as i32]).collect();
        let count = points.len();
        if count == 0 {
            return;
        }
        let mut e: Vec<Edge> = Vec::with_capacity(count);
        let mut i = 0;
        while i + 1 < count {
            let (x0, y0, x1, y1) = (xy[i * 2], xy[i * 2 + 1], xy[i * 2 + 2], xy[i * 2 + 3]);
            if y0 == y1 && i != 0 && y0 == xy[i * 2 - 1] {
                // A horizontal line that immediately follows another one.
                let last = e.last_mut().unwrap();
                if x1 > x0 && x0 > xy[i * 2 - 2] {
                    last.xmax = x1;
                    i += 1;
                    continue;
                } else if x1 < x0 && x0 < xy[i * 2 - 2] {
                    last.xmin = x1;
                    i += 1;
                    continue;
                }
            }
            e.push(edge(x0, y0, x1, y1));
            i += 1;
        }
        if xy[i * 2] != xy[0] || xy[i * 2 + 1] != xy[1] {
            e.push(edge(xy[i * 2], xy[i * 2 + 1], xy[0], xy[1]));
        }
        self.polygon_generic(&e);
    }

    fn polygon_generic(&mut self, e: &[Edge]) {
        if e.is_empty() {
            return;
        }
        let mut ymin = self.height - 1;
        let mut ymax = 0;
        let mut table: Vec<&Edge> = Vec::with_capacity(e.len());
        for edge in e {
            ymin = ymin.min(edge.ymin);
            ymax = ymax.max(edge.ymax);
            if edge.ymin == edge.ymax {
                self.hline(edge.xmin, edge.ymin, edge.xmax);
                continue;
            }
            table.push(edge);
        }
        ymin = ymin.max(0);
        ymax = ymax.min(self.height);
        let at = |edge: &Edge, y: i32| (y - edge.y0) as f32 * edge.dx + edge.x0 as f32;
        let mut xx: Vec<f32> = Vec::with_capacity(table.len() * 2);
        for y in ymin..=ymax {
            xx.clear();
            for (i, cur) in table.iter().enumerate() {
                if y < cur.ymin || y > cur.ymax {
                    continue;
                }
                xx.push(at(cur, y));
                if y == cur.ymax && y < ymax {
                    // Needed to draw consistent polygons.
                    xx.push(*xx.last().unwrap());
                } else if (y == cur.ymin || y == cur.ymax) && cur.dx != 0.0 {
                    // Connect discontiguous corners.
                    for other in &table[..i] {
                        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
                            continue;
                        }
                        let last = *xx.last().unwrap();
                        if last.round() != at(other, y).round() {
                            continue;
                        }
                        let offset = if y == cur.ymax { -1 } else { 1 };
                        let adj = at(cur, y + offset);
                        if y + offset >= other.ymin && y + offset <= other.ymax {
                            let adj_other = at(other, y + offset);
                            let slot = xx.last_mut().unwrap();
                            if last > adj + 1.0 && last > adj_other + 1.0 {
                                *slot = (f64::from(adj).max(f64::from(adj_other)) as f32).round() + 1.0;
                            } else if last < adj - 1.0 && last < adj_other - 1.0 {
                                *slot = (f64::from(adj).min(f64::from(adj_other)) as f32).round() - 1.0;
                            }
                            break;
                        }
                    }
                }
            }
            xx.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xx.as_chunks::<2>().0.iter() {
                self.hline(round_up(pair[0]), y, round_down(pair[1]));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(m: &Mask) -> Vec<String> {
        m.data
            .chunks(m.width as usize)
            .map(|r| r.iter().map(|&b| if b != 0 { '#' } else { '.' }).collect())
            .collect()
    }

    #[test]
    fn triangle_matches_pillow() {
        // Pillow 13: ImageDraw.Draw(Image.new("1", (6, 5))).polygon([(0, 0), (5, 0), (0, 4)], fill=1)
        let mut m = Mask::new(6, 5);
        m.fill_polygon(&[(0.0, 0.0), (5.0, 0.0), (0.0, 4.0)]);
        assert_eq!(rows(&m), ["######", "#####.", "###...", "##....", "#....."]);
    }

    #[test]
    fn quad_halves_cover_the_rect() {
        let mut m = Mask::new(4, 3);
        m.fill_polygon(&[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)]);
        m.fill_polygon(&[(0.0, 0.0), (4.0, 3.0), (0.0, 3.0)]);
        assert!(m.data.iter().all(|&b| b == 1));
    }
}
