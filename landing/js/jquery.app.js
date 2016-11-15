/* ==============================================
Smooth Scroll To Anchor
jQuery for page scrolling feature - requires jQuery Easing plugin
=============================================== */
$(function() {
    $('.navbar-nav a, .pseudo-link').bind('click', function(event) {
        var $link = $(this);
        var ahcnor = $link.attr('href');
        var $target = $(ahcnor);

        if ($target.length === 0) {
            return true;
        }

        event.preventDefault();

        // Scroll page to hash
        $('html, body').stop().animate({
            scrollTop: $target.offset().top - 78
        }, 1500, 'easeInOutExpo');

        // Add hash to URL
        if (history.pushState) {
            history.pushState(null, null, ahcnor);
        } else {
            location.hash = ahcnor;
        }
    });

    // Collapse navbar on navbar item click
    $('.navbar-nav a').bind('click', function() {
        $('.navbar-collapse').collapse('hide');
    });
});

/* ==============================================
Preloader
=============================================== */
$(window).load(function() {
    $('.status').fadeOut();
    $('.preloader').delay(350).fadeOut('slow');
});

/* ==============================================
WOW plugin triggers animate.css on scroll
=============================================== */
jQuery(document).ready(function () {
    wow = new WOW(
        {
            animateClass: 'animated',
            offset: 100,
            mobile: true
        }
    );
    wow.init();
});

/* ==============================================
 Sticky header on scroll
 =============================================== */
$(window).load(function() {
    $(".sticky").sticky({topSpacing: 0});
});

/* ==============================================
Contact App
=============================================== */

//var $ = jQuery.noConflict(); //Relinquish jQuery's control of the $ variable.
