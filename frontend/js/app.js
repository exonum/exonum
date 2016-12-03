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
            scrollTop: $target.offset().top
        }, 1000, 'easeInOutExpo');

        // Add hash to URL
        if (history.pushState) {
            history.pushState(null, null, ahcnor);
        } else {
            location.hash = ahcnor;
        }
    });

});