#ifndef __RSX_H__
#define __RSX_H__

#include "libretro.h"

#ifdef __cplusplus
extern "C" {
#endif

  void rsx_set_environment(retro_environment_t);
  void rsx_set_video_refresh(retro_video_refresh_t);
  void rsx_get_system_av_info(struct retro_system_av_info *);

  void rsx_init(void);
  bool rsx_open(bool is_pal);
  void rsx_close();
  void rsx_refresh_variables();
  void rsx_prepare_frame();
  void rsx_finalize_frame();

  void rsx_set_draw_offset(int16_t x, int16_t y);
  void rsx_set_draw_area(uint16_t x, uint16_t y,
			 uint16_t w, uint16_t h);
  void rsx_set_display_mode(uint16_t x, uint16_t y,
			    uint16_t w, uint16_t h,
			    bool depth_24bpp);

  void rsx_push_triangle(int16_t p0x, int16_t p0y,
			 int16_t p1x, int16_t p1y,
			 int16_t p2x, int16_t p2y,
			 int32_t c0,
			 int32_t c1,
			 int32_t c2,
			 bool dither);

#ifdef __cplusplus
}
#endif


#endif /*__RSX_H__ */
